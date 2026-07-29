//
// Copyright(C) 2005-2014 Simon Howard
// Copyright(C) 2026 Retro Deck contributors
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.
//
// DESCRIPTION:
//	Software OPL driver for Retro Deck, replacing Chocolate Doom's
//	SDL_mixer driver.
//
//	The register, timer, and callback-scheduling logic is the same as the
//	upstream SDL driver, because that is the behaviour i_oplmusic.c is
//	written against. Two things differ. First, output is pulled by the
//	sound mixer on the game thread rather than pushed from an audio
//	callback thread, so every mutex in the original collapses into
//	nothing: there is no second thread to exclude. Second, the OPL chip
//	can be left off, because emulating it costs real time on a 650 MHz
//	Cortex-A7 and a silent game is better than a stuttering one.
//

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "dbopl.h"
#include "opl.h"
#include "opl_internal.h"
#include "opl_queue.h"

#include "retrodeck_doom.h"
#include "retrodeck_opl.h"

// Set RETRO_DECK_DOOM_MUSIC=0 to leave the chip unemulated. Refusing in
// Init makes I_OPL_InitMusic fail, which leaves the engine with no music
// module at all rather than a half-initialised one.
//
// Which emulator does the work decides whether music is affordable here at
// all. Measured on the Deck, on screen, against the 28.6 ms budget at DOOM's
// 35 Hz tic rate: Chocolate Doom's later Nuked OPL3 held only 20 to 33 fps
// and shed most frames, because it always emulates at 49716 Hz internally
// whatever output rate it is handed, costing about 80% of a core and tuning
// down not at all. dbopl runs at the rate it is given and holds a flat
// 35.0 fps with no shed frames even at the full 44100 Hz.
#define MUSIC_ENVIRONMENT "RETRO_DECK_DOOM_MUSIC"

typedef struct
{
    unsigned int rate;        // Number of times the timer is advanced per sec.
    unsigned int enabled;     // Non-zero if timer is enabled.
    unsigned int value;       // Last value that was set.
    uint64_t expire_time;     // Calculated time that timer will expire.
} opl_timer_t;

// The chip runs at the mixer's rate by default, which measures free, and the
// output is then a straight copy. RETRO_DECK_DOOM_MUSIC_RATE lowers it and
// upsamples instead, trading quality for CPU; kept because it costs a few
// lines and is the first knob to reach for if the budget ever tightens.
#define RATE_ENVIRONMENT "RETRO_DECK_DOOM_MUSIC_RATE"
#define MINIMUM_CHIP_RATE 4000

static opl_callback_queue_t *callback_queue = NULL;
static uint64_t current_time = 0;
static int opl_paused = 0;
static uint64_t pause_offset = 0;

// The rate the chip is emulated at, and the rate the mixer wants.
static unsigned int chip_rate = 0;
static unsigned int mixer_rate = 0;

// 16.16 read position between previous_frame and current_frame, plus the
// two chip frames being interpolated, so playback stays continuous across
// calls. Starting at one whole frame makes the first output pull a frame.
static uint32_t resample_position = 0x10000;
static uint32_t resample_step = 0;
static int16_t previous_frame[2];
static int16_t current_frame[2];

static Chip opl_chip;
static int opl_opl3mode = 0;
static int register_num = 0;
static int driver_initialized = 0;

// dbopl emits 32-bit samples, generated in batches because its per-call
// setup would otherwise dominate at one frame at a time. Frames are then
// handed out one at a time to the resampler.
#define CHIP_BATCH_FRAMES 256

static Bit32s chip_batch[CHIP_BATCH_FRAMES * 2];
static unsigned int chip_available = 0;
static unsigned int chip_consumed = 0;
// Latched when the batch is generated: a register write can switch the chip
// between OPL2 and OPL3 mid-batch, which would change how it unpacks.
static int batch_opl3mode = 0;

static opl_timer_t timer1 = { 12500, 0, 0, 0 };
static opl_timer_t timer2 = { 3125, 0, 0, 0 };

// Callbacks must not run while the music module holds the lock, matching
// OPL_Lock's contract. Single-threaded, so a counter is enough.
static int callbacks_locked = 0;

// Frames the chip rendered with any signal in them. The host reports this
// in test mode: if the MUS-to-MIDI conversion or the instrument table were
// broken, the chip would run but emit nothing but silence.
static unsigned int voiced_frames = 0;
static unsigned int register_writes = 0;
static unsigned int callback_schedules = 0;

int retrodeck_opl_active(void)
{
    return driver_initialized;
}

static int MusicEnabled(void)
{
    const char *setting = getenv(MUSIC_ENVIRONMENT);

    return setting == NULL || strcmp(setting, "0") != 0;
}

static void WriteRegister(unsigned int reg_num, unsigned int value);

//
// Put the chip into the state DOOM's music code assumes.
//
// opl.c exposes OPL_InitRegisters for this, but it dispatches through the
// driver pointer, which OPL_Init only assigns after this init function
// returns; and fbDOOM's i_oplmusic never calls it in the first place. Nuked
// OPL3 happened to sound anyway. dbopl does not: without the waveform-enable
// write below, every voice stays silent. So the OPL2 half of that routine is
// performed here, straight against the chip.
//

static void InitChipRegisters(void)
{
    int r;

    for (r = OPL_REGS_LEVEL; r <= OPL_REGS_LEVEL + OPL_NUM_OPERATORS; ++r)
    {
        WriteRegister(r, 0x3f);
    }

    // Both loops deliberately touch registers that do not exist, and the
    // bounds are deliberately inclusive: this is what DOOM itself does.
    for (r = OPL_REGS_ATTACK; r <= OPL_REGS_WAVEFORM + OPL_NUM_OPERATORS; ++r)
    {
        WriteRegister(r, 0x00);
    }

    for (r = 1; r < OPL_REGS_LEVEL; ++r)
    {
        WriteRegister(r, 0x00);
    }

    // Reset both timers and enable interrupts.
    WriteRegister(OPL_REG_TIMER_CTRL, 0x60);
    WriteRegister(OPL_REG_TIMER_CTRL, 0x80);

    // Let the FM chip control each operator's waveform. This is the write
    // dbopl needs before it will produce anything at all.
    WriteRegister(OPL_REG_WAVEFORM_ENABLE, 0x20);
}

static int OPL_Retrodeck_Init(unsigned int port_base)
{
    (void) port_base;

    if (!MusicEnabled())
    {
        retrodeck_doom_log("music disabled by " MUSIC_ENVIRONMENT);
        return 0;
    }
    if (opl_sample_rate == 0)
    {
        return 0;
    }

    callback_queue = OPL_Queue_Create();
    if (callback_queue == NULL)
    {
        return 0;
    }

    current_time = 0;
    pause_offset = 0;
    opl_paused = 0;
    register_num = 0;
    opl_opl3mode = 0;
    callbacks_locked = 0;

    mixer_rate = opl_sample_rate;
    chip_rate = mixer_rate;
    {
        const char *requested = getenv(RATE_ENVIRONMENT);

        if (requested != NULL)
        {
            int value = atoi(requested);

            if (value >= MINIMUM_CHIP_RATE)
            {
                chip_rate = (unsigned int) value;
            }
        }
    }
    if (chip_rate > mixer_rate)
    {
        chip_rate = mixer_rate;
    }

    resample_position = 0x10000;
    resample_step = (uint32_t) (((uint64_t) chip_rate << 16) / mixer_rate);
    previous_frame[0] = 0;
    previous_frame[1] = 0;
    current_frame[0] = 0;
    current_frame[1] = 0;

    chip_available = 0;
    chip_consumed = 0;
    DBOPL_InitTables();
    Chip__Chip(&opl_chip);
    Chip__Setup(&opl_chip, chip_rate);
    driver_initialized = 1;
    InitChipRegisters();

    {
        char message[96];
        snprintf(message, sizeof(message),
                 "OPL music ready, chip at %u Hz upsampled to %u Hz",
                 chip_rate, mixer_rate);
        retrodeck_doom_log(message);
    }
    return 1;
}

static void OPL_Retrodeck_Shutdown(void)
{
    driver_initialized = 0;
    if (callback_queue != NULL)
    {
        OPL_Queue_Destroy(callback_queue);
        callback_queue = NULL;
    }
}

static unsigned int OPL_Retrodeck_PortRead(opl_port_t port)
{
    unsigned int result = 0;


    if (port == OPL_REGISTER_PORT_OPL3)
    {
        return 0xff;
    }

    if (timer1.enabled && current_time > timer1.expire_time)
    {
        result |= 0x80;   // Either have expired
        result |= 0x40;   // Timer 1 has expired
    }

    if (timer2.enabled && current_time > timer2.expire_time)
    {
        result |= 0x80;   // Either have expired
        result |= 0x20;   // Timer 2 has expired
    }

    return result;
}

static void OPLTimer_CalculateEndTime(opl_timer_t *timer)
{
    int tics;

    if (timer->enabled)
    {
        tics = 0x100 - timer->value;
        timer->expire_time = current_time
                           + ((uint64_t) tics * OPL_SECOND) / timer->rate;
    }
}

static void WriteRegister(unsigned int reg_num, unsigned int value)
{
    switch (reg_num)
    {
        case OPL_REG_TIMER1:
            timer1.value = value;
            OPLTimer_CalculateEndTime(&timer1);
            break;

        case OPL_REG_TIMER2:
            timer2.value = value;
            OPLTimer_CalculateEndTime(&timer2);
            break;

        case OPL_REG_TIMER_CTRL:
            if (value & 0x80)
            {
                timer1.enabled = 0;
                timer2.enabled = 0;
            }
            else
            {
                if ((value & 0x40) == 0)
                {
                    timer1.enabled = (value & 0x01) != 0;
                    OPLTimer_CalculateEndTime(&timer1);
                }

                if ((value & 0x20) == 0)
                {
                    timer2.enabled = (value & 0x02) != 0;
                    OPLTimer_CalculateEndTime(&timer2);
                }
            }

            break;

        case OPL_REG_NEW:
            opl_opl3mode = value & 0x01;
            /* fall through */

        default:
            ++register_writes;
            Chip__WriteReg(&opl_chip, (Bit32u) reg_num, (Bit8u) value);
            break;
    }
}

static void OPL_Retrodeck_PortWrite(opl_port_t port, unsigned int value)
{
    if (port == OPL_REGISTER_PORT)
    {
        register_num = value;
    }
    else if (port == OPL_REGISTER_PORT_OPL3)
    {
        register_num = value | 0x100;
    }
    else if (port == OPL_DATA_PORT)
    {
        WriteRegister(register_num, value);
    }
}

static void OPL_Retrodeck_SetCallback(uint64_t us, opl_callback_t callback,
                                      void *data)
{
    if (callback_queue == NULL)
    {
        return;
    }
    ++callback_schedules;
    OPL_Queue_Push(callback_queue, callback, data,
                   current_time - pause_offset + us);
}

static void OPL_Retrodeck_ClearCallbacks(void)
{
    if (callback_queue != NULL)
    {
        OPL_Queue_Clear(callback_queue);
    }
}

static void OPL_Retrodeck_Lock(void)
{
    // No audio thread exists, so this only has to stop the mixer from
    // running callbacks while the music module rewrites its state. The
    // mixer and the music module both run on the game thread, so a flag
    // is sufficient where the SDL driver needed a mutex.
    ++callbacks_locked;
}

static void OPL_Retrodeck_Unlock(void)
{
    if (callbacks_locked > 0)
    {
        --callbacks_locked;
    }
}

static void OPL_Retrodeck_SetPaused(int paused)
{
    opl_paused = paused;
}

static void OPL_Retrodeck_AdjustCallbacks(float factor)
{
    if (callback_queue != NULL)
    {
        OPL_Queue_AdjustCallbacks(callback_queue, current_time, factor);
    }
}

//
// Advance the emulated clock and run any callbacks that come due.
//

static void AdvanceTime(unsigned int nsamples)
{
    opl_callback_t callback;
    void *callback_data;
    uint64_t us;

    us = ((uint64_t) nsamples * OPL_SECOND) / chip_rate;
    current_time += us;

    if (opl_paused)
    {
        pause_offset += us;
        return;
    }

    while (!callbacks_locked
        && !OPL_Queue_IsEmpty(callback_queue)
        && current_time >= OPL_Queue_Peek(callback_queue) + pause_offset)
    {
        if (!OPL_Queue_Pop(callback_queue, &callback, &callback_data))
        {
            break;
        }

        callback(callback_data);
    }
}

// Pull one chip frame, advancing the emulated clock by exactly that frame so
// music callbacks land on the sample they are scheduled for. This replaces
// the upstream driver's span splitting: at one-frame granularity the
// callback queue is checked more often than any span boundary would.

// Refill the batch, stopping where the next music callback is due so note
// changes land on the sample they were scheduled for.

static void RefillChipBatch(void)
{
    unsigned int span = CHIP_BATCH_FRAMES;

    if (!opl_paused && !OPL_Queue_IsEmpty(callback_queue))
    {
        uint64_t next = OPL_Queue_Peek(callback_queue) + pause_offset;

        if (next <= current_time)
        {
            // Already due. Run it, then generate at least one frame so a
            // callback rescheduling itself for now cannot spin forever.
            AdvanceTime(0);
            span = 1;
        }
        else
        {
            uint64_t frames =
                ((next - current_time) * chip_rate + OPL_SECOND - 1)
                / OPL_SECOND;

            if (frames < span)
            {
                span = frames == 0 ? 1 : (unsigned int) frames;
            }
        }
    }

    batch_opl3mode = opl_opl3mode;
    if (batch_opl3mode)
    {
        Chip__GenerateBlock3(&opl_chip, span, chip_batch);
    }
    else
    {
        // OPL2 output is mono; the resampler expects interleaved stereo, so
        // spread it while unpacking below.
        Chip__GenerateBlock2(&opl_chip, span, chip_batch);
    }

    chip_available = span;
    chip_consumed = 0;
}

static void NextChipFrame(void)
{
    Bit32s left;
    Bit32s right;

    previous_frame[0] = current_frame[0];
    previous_frame[1] = current_frame[1];

    if (chip_consumed >= chip_available)
    {
        RefillChipBatch();
    }

    if (batch_opl3mode)
    {
        left = chip_batch[chip_consumed * 2];
        right = chip_batch[chip_consumed * 2 + 1];
    }
    else
    {
        left = chip_batch[chip_consumed];
        right = left;
    }
    ++chip_consumed;

    if (left > 32767) { left = 32767; }
    if (left < -32768) { left = -32768; }
    if (right > 32767) { right = 32767; }
    if (right < -32768) { right = -32768; }

    current_frame[0] = (int16_t) left;
    current_frame[1] = (int16_t) right;

    if (left != 0 || right != 0)
    {
        ++voiced_frames;
    }

    // Per consumed frame, never per generated batch. Advancing by the batch
    // instead lets a caller consume frames already in hand without the clock
    // moving at all, which stalls the timers the chip-detection handshake
    // reads back and skews every scheduled music callback.
    AdvanceTime(1);
}

// Always leaves the whole buffer written, so the sound mixer can sum
// effects on top without clearing it first.

void retrodeck_opl_generate(int16_t *buffer, int frames)
{
    int index;

    if (buffer == NULL || frames <= 0)
    {
        return;
    }
    if (!driver_initialized)
    {
        memset(buffer, 0, (size_t) frames * 2 * sizeof(int16_t));
        return;
    }

    for (index = 0; index < frames; ++index)
    {
        int32_t weight;

        // Consume whole chip frames until the read position falls inside
        // the span between the previous frame and the current one.
        while (resample_position >= 0x10000)
        {
            NextChipFrame();
            resample_position -= 0x10000;
        }

        // Linear interpolation. Sample-and-hold would be cheaper, but at a
        // 4:1 ratio its imaging is audible on sustained notes.
        weight = (int32_t) resample_position;
        buffer[index * 2] = (int16_t)
            (previous_frame[0]
             + (((int32_t) current_frame[0] - previous_frame[0]) * weight
                >> 16));
        buffer[index * 2 + 1] = (int16_t)
            (previous_frame[1]
             + (((int32_t) current_frame[1] - previous_frame[1]) * weight
                >> 16));

        resample_position += resample_step;
    }
}

int retrodeck_opl_voiced_spans(void)
{
    return (int) voiced_frames;
}

int retrodeck_opl_register_writes(void)
{
    return (int) register_writes;
}

int retrodeck_opl_callback_schedules(void)
{
    return (int) callback_schedules;
}

void retrodeck_opl_delay(uint64_t microseconds)
{
    // The SDL driver could block here because its audio thread kept
    // advancing the chip. Nothing advances it in a pull model, so the wait
    // becomes emulation whose output is thrown away. Measured in chip
    // frames, since that is what carries the clock forward.
    uint64_t frames;

    if (!driver_initialized)
    {
        return;
    }

    frames = (microseconds * chip_rate + OPL_SECOND - 1) / OPL_SECOND;

    while (frames > 0)
    {
        NextChipFrame();
        --frames;
    }
}

opl_driver_t opl_retrodeck_driver =
{
    "Retrodeck",
    OPL_Retrodeck_Init,
    OPL_Retrodeck_Shutdown,
    OPL_Retrodeck_PortRead,
    OPL_Retrodeck_PortWrite,
    OPL_Retrodeck_SetCallback,
    OPL_Retrodeck_ClearCallbacks,
    OPL_Retrodeck_Lock,
    OPL_Retrodeck_Unlock,
    OPL_Retrodeck_SetPaused,
    OPL_Retrodeck_AdjustCallbacks,
};
