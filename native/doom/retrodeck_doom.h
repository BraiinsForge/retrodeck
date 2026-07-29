//
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
//	Boundary between the DOOM engine and the Rust doom-host frontend.
//	Every function here is implemented in Rust; the DOOM platform
//	backends in this directory are the only callers.
//

#ifndef __RETRODECK_DOOM__
#define __RETRODECK_DOOM__

#include <stdint.h>

// One translated input transition. The Rust host owns the controller and
// keyboard mapping tables, so the engine side never sees evdev codes.

typedef struct
{
    // Key code from doomkeys.h. Never zero.
    int32_t key;

    // Shift-translated ASCII character for a press, or zero when the key
    // has no printable meaning. Always zero for a release.
    int32_t character;

    // Nonzero for a press, zero for a release.
    int32_t pressed;
} retrodeck_doom_event_t;

// Present one finished 320x200 frame as packed XRGB8888 and pace the
// frame. Returns zero on success and nonzero when the display failed and
// DOOM should quit.
int retrodeck_doom_present(const uint32_t *pixels, int width, int height);

// Drain pending input transitions into events, returning the count
// written. Never writes more than max_events entries.
int retrodeck_doom_poll_events(retrodeck_doom_event_t *events, int max_events);

// Nonzero once the supervising dashboard, a signal, or a failed present
// has asked DOOM to exit.
int retrodeck_doom_quit_requested(void);

// Milliseconds since the host started. Derived from the presented frame
// count in test mode so recorded demos hash reproducibly.
int retrodeck_doom_ticks(void);

// Sleep, clamped by the host. A no-op in test mode.
void retrodeck_doom_sleep(int milliseconds);

// Queue interleaved stereo frames at the host sample rate.
void retrodeck_doom_audio_write(const int16_t *frames, int frame_count);

// Host output sample rate, or zero when sound is unavailable.
int retrodeck_doom_audio_rate(void);

// Frames already queued for playback. The mixer tops the queue up to a
// target depth instead of assuming it is called at an exact rate.
int retrodeck_doom_audio_queued(void);

// Write one diagnostic line to the dashboard log.
void retrodeck_doom_log(const char *message);

// Release the display, controllers, and audio, then leave. Never returns.
// The engine's own I_Quit runs its exit handlers but, in this fork, its
// exit() is compiled out, so it returns to its caller instead of leaving.
void retrodeck_doom_exit(int status);

// Shared between the backends in this directory. Upstream fbDOOM declares
// I_InitInput inconsistently across files, so it is declared once here.

#include "doomtype.h"

void I_InitInput(void);

// True once the host asked DOOM to quit or a present failed. Defined by
// the video backend and read by the input backend's tic handler.
boolean I_RetrodeckQuitRequested(void);

#endif
