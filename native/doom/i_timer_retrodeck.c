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
//	Timing, read from the Rust host instead of gettimeofday. The host
//	substitutes a frame-derived clock in test mode, which is what makes
//	a pinned frame hash reproducible.
//

#include "i_timer.h"
#include "doomtype.h"

#include "retrodeck_doom.h"

//
// I_GetTime
// returns time in 1/35th second tics
//

int I_GetTime (void)
{
    return (retrodeck_doom_ticks() * TICRATE) / 1000;
}

//
// Same as I_GetTime, but returns time in milliseconds
//

int I_GetTimeMS (void)
{
    return retrodeck_doom_ticks();
}

// Sleep for a specified number of ms

void I_Sleep(int ms)
{
    retrodeck_doom_sleep(ms);
}

void I_WaitVBL(int count)
{
    I_Sleep((count * 1000) / 70);
}

void I_InitTimer(void)
{
    // The host clock is already running when DOOM starts, and it is the
    // only clock, so there is no base time to capture here.
}
