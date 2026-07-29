//
// Copyright(C) 1993-1996 Id Software, Inc.
// Copyright(C) 2005-2014 Simon Howard
// Copyright(C) 2026 Retro Deck contributors
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// DESCRIPTION:
//	Sound effects for Retro Deck. fbDOOM builds with no sound module at
//	all, and the Chocolate Doom module it inherited needs SDL, SDL_mixer,
//	and libsamplerate. This one needs none of them: it keeps DOOM's own
//	11 kHz DMX lumps as they are and point-resamples them into the host's
//	stereo queue at mix time, so no memory goes on resampled copies.
//
//	Mixing happens in Update, which S_UpdateSounds calls once per rendered
//	frame. Rather than assume an exact call rate, each pass tops the host
//	queue up to a target depth, which absorbs a shed frame without
//	draining the queue to silence.
//

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "doomtype.h"
#include "i_sound.h"
#include "i_swap.h"
#include "m_misc.h"
#include "w_wad.h"
#include "z_zone.h"

#include "retrodeck_doom.h"
#include "retrodeck_opl.h"

// DOOM's default is 8; snd_channels is configurable, so leave headroom and
// reject anything past it rather than writing out of bounds.
#define MAX_CHANNELS 32

// Roughly 70 ms of audio in flight. Enough to survive a shed frame on the
// dual Cortex-A7, short enough that a shotgun still feels immediate.
#define QUEUE_TARGET_DIVISOR 14

// Never mix more than about two tics of audio in one pass. Beyond bounding
// the catch-up after a stall, this matters when the host has no open audio
// device: the queue then reads as permanently empty, and without a cap every
// pass would mix a full target's worth: two and a half times what playback
// actually consumes, wasting exactly the CPU that music needs.
#define QUEUE_BATCH_DIVISOR 17

#define MIXING_FRAMES 2048

// DOOM's own effects are 11025 Hz. These bounds accept anything a real WAD
// plausibly carries while rejecting values that would break resampling.
#define MINIMUM_SFX_RATE 1000
#define MAXIMUM_SFX_RATE 96000

typedef struct
{
    const uint8_t *samples;
    uint32_t length;
    uint32_t rate;
} cached_sound_t;

typedef struct
{
    const cached_sound_t *sound;
    // 16.16 fixed-point read position and per-output-frame step.
    uint32_t position;
    uint32_t step;
    int left_volume;
    int right_volume;
    boolean active;
} mix_channel_t;

static boolean sound_initialized = false;
static boolean use_sfx_prefix_setting = true;
static int output_rate = 0;

// Counts effects that were successfully cached and started. The host reports
// it in test mode, which is what proves the DMX parsing ran rather than the
// mixer having quietly produced silence.
static int started_count = 0;
static mix_channel_t channels[MAX_CHANNELS];
static int16_t mixing_buffer[MIXING_FRAMES * 2];

static snddevice_t sound_devices[] =
{
    SNDDEVICE_SB,
};

//
// Parse one DMX sound lump. The format is a four-word header followed by
// unsigned 8-bit PCM. The first and last 16 samples are padding that the
// original tools inserted; skipping them avoids a click at both ends.
//

static boolean CacheSound(sfxinfo_t *sfxinfo)
{
    const uint8_t *lump;
    cached_sound_t *cached;
    uint32_t rate;
    uint32_t length;
    int lump_length;

    if (sfxinfo->driver_data != NULL)
    {
        return true;
    }

    if (sfxinfo->lumpnum < 0)
    {
        sfxinfo->lumpnum = I_GetSfxLumpNum(sfxinfo);
        if (sfxinfo->lumpnum < 0)
        {
            return false;
        }
    }

    lump_length = W_LumpLength(sfxinfo->lumpnum);
    if (lump_length < 8)
    {
        return false;
    }

    lump = (const uint8_t *) W_CacheLumpNum(sfxinfo->lumpnum, PU_STATIC);
    if (lump == NULL)
    {
        return false;
    }

    // Format 3 is the only digital sound format DOOM ships.
    if (lump[0] != 0x03 || lump[1] != 0x00)
    {
        return false;
    }

    rate = (uint32_t) lump[2] | ((uint32_t) lump[3] << 8);
    length = (uint32_t) lump[4] | ((uint32_t) lump[5] << 8)
           | ((uint32_t) lump[6] << 16) | ((uint32_t) lump[7] << 24);

    // A rate low enough to round the resampling step down to zero would
    // hold a channel open forever on the same sample, so bound it rather
    // than trusting whatever an arbitrary PWAD declares.
    if (rate < MINIMUM_SFX_RATE || rate > MAXIMUM_SFX_RATE
        || length <= 48 || length > (uint32_t) lump_length - 8)
    {
        return false;
    }

    cached = (cached_sound_t *) Z_Malloc(sizeof(cached_sound_t),
                                        PU_STATIC, NULL);
    cached->samples = lump + 8 + 16;
    cached->length = length - 32;
    cached->rate = rate;
    sfxinfo->driver_data = cached;
    return true;
}

static boolean I_Retrodeck_InitSound(boolean use_sfx_prefix)
{
    output_rate = retrodeck_doom_audio_rate();
    if (output_rate <= 0)
    {
        retrodeck_doom_log("sound effects disabled: the host has no audio");
        return false;
    }

    use_sfx_prefix_setting = use_sfx_prefix;
    memset(channels, 0, sizeof(channels));
    sound_initialized = true;

    {
        char message[64];
        snprintf(message, sizeof(message), "sound effects ready at %d Hz",
                 output_rate);
        retrodeck_doom_log(message);
    }
    return true;
}

static void I_Retrodeck_ShutdownSound(void)
{
    sound_initialized = false;
    memset(channels, 0, sizeof(channels));
}

static int I_Retrodeck_GetSfxLumpNum(sfxinfo_t *sfxinfo)
{
    char name[9];

    if (use_sfx_prefix_setting)
    {
        snprintf(name, sizeof(name), "ds%s", sfxinfo->name);
    }
    else
    {
        snprintf(name, sizeof(name), "%s", sfxinfo->name);
    }

    return W_CheckNumForName(name);
}

//
// The vanilla stereo separation curve, kept so panning sounds like DOOM
// rather than like a modern mixer. vol arrives as 0-127 and sep as 0-254.
//

static void SetChannelVolume(mix_channel_t *channel, int vol, int sep)
{
    int separation = sep + 1;

    channel->left_volume = vol - ((vol * separation * separation) >> 16);
    separation = 257 - separation;
    channel->right_volume = vol - ((vol * separation * separation) >> 16);

    if (channel->left_volume < 0)
    {
        channel->left_volume = 0;
    }
    if (channel->right_volume < 0)
    {
        channel->right_volume = 0;
    }
}

static int I_Retrodeck_StartSound(sfxinfo_t *sfxinfo, int channel, int vol,
                                  int sep)
{
    mix_channel_t *slot;

    if (!sound_initialized || channel < 0 || channel >= MAX_CHANNELS)
    {
        return -1;
    }
    if (!CacheSound(sfxinfo))
    {
        return -1;
    }

    slot = &channels[channel];
    slot->sound = (const cached_sound_t *) sfxinfo->driver_data;
    slot->position = 0;
    slot->step = (uint32_t) (((uint64_t) slot->sound->rate << 16)
                             / (uint32_t) output_rate);
    SetChannelVolume(slot, vol, sep);
    slot->active = true;
    ++started_count;
    return channel;
}

int retrodeck_doom_sfx_started(void)
{
    return started_count;
}

static void I_Retrodeck_StopSound(int channel)
{
    if (channel < 0 || channel >= MAX_CHANNELS)
    {
        return;
    }
    channels[channel].active = false;
}

static boolean I_Retrodeck_SoundIsPlaying(int channel)
{
    if (channel < 0 || channel >= MAX_CHANNELS)
    {
        return false;
    }
    return channels[channel].active;
}

static void I_Retrodeck_UpdateSoundParams(int channel, int vol, int sep)
{
    if (channel < 0 || channel >= MAX_CHANNELS || !channels[channel].active)
    {
        return;
    }
    SetChannelVolume(&channels[channel], vol, sep);
}

//
// Mix one batch and hand it to the host.
//

static void I_Retrodeck_UpdateSound(void)
{
    int target;
    int queued;
    int wanted;

    if (!sound_initialized)
    {
        return;
    }

    target = output_rate / QUEUE_TARGET_DIVISOR;
    queued = retrodeck_doom_audio_queued();
    wanted = target - queued;

    if (wanted <= 0)
    {
        return;
    }
    if (wanted > output_rate / QUEUE_BATCH_DIVISOR)
    {
        wanted = output_rate / QUEUE_BATCH_DIVISOR;
    }

    while (wanted > 0)
    {
        int frames = wanted > MIXING_FRAMES ? MIXING_FRAMES : wanted;
        int index;
        int slot;

        // Music first: the OPL driver fills the whole span, silence
        // included, then effects are summed on top of it. One buffer, one
        // path into the host queue.
        retrodeck_opl_generate(mixing_buffer, frames);

        for (slot = 0; slot < MAX_CHANNELS; ++slot)
        {
            mix_channel_t *channel = &channels[slot];
            const cached_sound_t *sound = channel->sound;
            uint32_t position;
            uint32_t limit;

            if (!channel->active || sound == NULL)
            {
                continue;
            }

            position = channel->position;
            limit = sound->length << 16;

            for (index = 0; index < frames; ++index)
            {
                int sample;
                int left;
                int right;

                if (position >= limit)
                {
                    channel->active = false;
                    break;
                }

                // Unsigned 8-bit centred on 128, widened to 16-bit.
                sample = ((int) sound->samples[position >> 16] - 128) << 8;
                position += channel->step;

                left = mixing_buffer[index * 2]
                     + ((sample * channel->left_volume) >> 7);
                right = mixing_buffer[index * 2 + 1]
                      + ((sample * channel->right_volume) >> 7);

                if (left > 32767) { left = 32767; }
                if (left < -32768) { left = -32768; }
                if (right > 32767) { right = 32767; }
                if (right < -32768) { right = -32768; }

                mixing_buffer[index * 2] = (int16_t) left;
                mixing_buffer[index * 2 + 1] = (int16_t) right;
            }

            channel->position = position;
        }

        retrodeck_doom_audio_write(mixing_buffer, frames);
        wanted -= frames;
    }
}

static void I_Retrodeck_CacheSounds(sfxinfo_t *sounds, int num_sounds)
{
    // Deliberately lazy. Precaching every effect would parse the whole sfx
    // set at startup for no benefit: the lumps are used in place, so the
    // first use costs one header parse and nothing is copied.
    (void) sounds;
    (void) num_sounds;
}

sound_module_t sound_retrodeck_module =
{
    sound_devices,
    arrlen(sound_devices),
    I_Retrodeck_InitSound,
    I_Retrodeck_ShutdownSound,
    I_Retrodeck_GetSfxLumpNum,
    I_Retrodeck_UpdateSound,
    I_Retrodeck_UpdateSoundParams,
    I_Retrodeck_StartSound,
    I_Retrodeck_StopSound,
    I_Retrodeck_SoundIsPlaying,
    I_Retrodeck_CacheSounds,
};
