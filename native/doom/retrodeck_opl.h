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
//	Boundary between the Retro Deck OPL driver and the sound mixer that
//	pulls it. Chocolate Doom's OPL layer expects an audio callback thread
//	to drive it; here the sound module pulls instead, on the game thread.
//

#ifndef __RETRODECK_OPL__
#define __RETRODECK_OPL__

#include <stdint.h>

// Nonzero once the software OPL is initialised and should be mixed.
int retrodeck_opl_active(void);

// Render frames of interleaved stereo OPL output, advancing the emulated
// chip's clock and running any music callbacks that fall inside the span.
void retrodeck_opl_generate(int16_t *buffer, int frames);

// Advance the emulated chip by a span with no listener. OPL_Delay maps onto
// this: chip detection writes a timer register and then reads the status
// back, which only changes once emulated time has moved.
void retrodeck_opl_delay(uint64_t microseconds);

#endif
