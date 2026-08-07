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
//	DOOM graphics for Retro Deck. DOOM keeps drawing into its own 320x200
//	paletted buffer; this backend expands that through the current palette
//	and hands one XRGB8888 frame to the Rust host, which owns scaling,
//	rotation, the exit cross, and both the normal BMC widget and framebuffer
//	presentation paths. None of the display geometry lives here.
//

#include <limits.h>
#include <stdlib.h>
#include <string.h>

#include "config.h"
#include "d_event.h"
#include "d_main.h"
#include "i_system.h"
#include "i_video.h"
#include "tables.h"
#include "v_video.h"
#include "z_zone.h"

#include "retrodeck_doom.h"

// Mouse is not reachable on the Deck; v_video.c and i_video.c read this.

int usemouse = 0;

byte *I_VideoBuffer = NULL;

boolean screensaver_mode = false;
boolean screenvisible;

float mouse_acceleration = 2.0;
int mouse_threshold = 10;

// Gamma correction level to use

int usegamma = 0;

// Palette expanded to the host's pixel format, gamma already applied.

static uint32_t xrgb_palette[256];

// One frame of expanded pixels, reused every present.

static uint32_t *frame_pixels = NULL;

// Latched once the host reports a failed present, so the engine leaves
// through its own shutdown path instead of dying inside a draw.

static boolean present_failed = false;

void I_InitGraphics (void)
{
    I_VideoBuffer = (byte *) Z_Malloc (SCREENWIDTH * SCREENHEIGHT,
                                       PU_STATIC, NULL);

    frame_pixels = (uint32_t *) malloc (SCREENWIDTH * SCREENHEIGHT
                                        * sizeof (uint32_t));
    if (frame_pixels == NULL)
    {
        I_Error ("I_InitGraphics: cannot allocate the presentation frame");
    }
    memset (frame_pixels, 0, SCREENWIDTH * SCREENHEIGHT * sizeof (uint32_t));

    screenvisible = true;

    I_InitInput ();
}

void I_ShutdownGraphics (void)
{
    if (I_VideoBuffer != NULL)
    {
        Z_Free (I_VideoBuffer);
        I_VideoBuffer = NULL;
    }

    free (frame_pixels);
    frame_pixels = NULL;
}

void I_StartFrame (void)
{
}

void I_UpdateNoBlit (void)
{
}

//
// I_FinishUpdate
//

void I_FinishUpdate (void)
{
    const byte *source;
    uint32_t *destination;
    int pixels;
    int i;

    if (frame_pixels == NULL || present_failed)
    {
        return;
    }

    source = I_VideoBuffer;
    destination = frame_pixels;
    pixels = SCREENWIDTH * SCREENHEIGHT;

    for (i = 0; i < pixels; ++i)
    {
        destination[i] = xrgb_palette[source[i]];
    }

    if (retrodeck_doom_present (frame_pixels, SCREENWIDTH, SCREENHEIGHT) != 0)
    {
        // The host has already reported why. Ask for a clean shutdown on
        // the next tic rather than tearing down the display mid-frame.
        present_failed = true;
    }
}

void I_ReadScreen (byte* scr)
{
    memcpy (scr, I_VideoBuffer, SCREENWIDTH * SCREENHEIGHT);
}

//
// I_SetPalette
//

void I_SetPalette (byte *palette)
{
    int i;

    for (i = 0; i < 256; ++i)
    {
        uint32_t r = gammatable[usegamma][*palette++];
        uint32_t g = gammatable[usegamma][*palette++];
        uint32_t b = gammatable[usegamma][*palette++];

        xrgb_palette[i] = (r << 16) | (g << 8) | b;
    }
}

// Given an RGB value, find the closest matching palette index.

int I_GetPaletteIndex (int r, int g, int b)
{
    int best, best_diff, diff;
    int i;

    best = 0;
    best_diff = INT_MAX;

    for (i = 0; i < 256; ++i)
    {
        int pr = (xrgb_palette[i] >> 16) & 0xFF;
        int pg = (xrgb_palette[i] >> 8) & 0xFF;
        int pb = xrgb_palette[i] & 0xFF;

        diff = (r - pr) * (r - pr)
             + (g - pg) * (g - pg)
             + (b - pb) * (b - pb);

        if (diff < best_diff)
        {
            best = i;
            best_diff = diff;
        }

        if (diff == 0)
        {
            break;
        }
    }

    return best;
}

// True once the host asked to quit or a present failed, so the input
// backend can post ev_quit from the tic handler.

boolean I_RetrodeckQuitRequested (void)
{
    return present_failed || retrodeck_doom_quit_requested () != 0;
}

void I_BeginRead (void)
{
}

void I_EndRead (void)
{
}

void I_SetWindowTitle (char *title)
{
}

void I_GraphicsCheckCommandLine (void)
{
}

void I_SetGrabMouseCallback (grabmouse_callback_t func)
{
}

void I_EnableLoadingDisk (void)
{
}

void I_BindVideoVariables (void)
{
}

void I_DisplayFPSDots (boolean dots_on)
{
}

void I_CheckIsScreensaver (void)
{
}
