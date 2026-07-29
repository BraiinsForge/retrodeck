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
//	DOOM input for Retro Deck. The Rust host owns evdev, the two
//	THEGamepad controllers, and every mapping table; this backend only
//	turns already-translated transitions into DOOM events.
//

#include "d_event.h"
#include "doomkeys.h"
#include "doomtype.h"
#include "i_system.h"
#include "i_video.h"
#include "m_controls.h"

#include "retrodeck_doom.h"

// Weapon cycling has no vanilla key. These two codes are unused by
// doomkeys.h, which reserves 0xa0 through 0xa3 for the movement pseudo-keys
// and 0xac through 0xaf for the arrows, leaving 0xa4 and 0xa5 free.

#define RETRODECK_KEY_PREVWEAPON 0xa4
#define RETRODECK_KEY_NEXTWEAPON 0xa5

// g_game.c treats any joybspeed at or above its MAX_JOY_BUTTONS as
// "always run", which is what makes a D-pad playable.

#define RETRODECK_ALWAYS_RUN 20

// The host reports shift-translated characters, matching what DOOM expects
// in data2, so no vanilla remapping happens here.

int vanilla_keyboard_mapping = 1;

// Drained per tic. Two controllers plus a keyboard cannot plausibly
// produce more transitions than this between tics.

#define MAX_EVENTS_PER_TIC 64

static boolean leaving = false;

void I_GetEvent (void)
{
    retrodeck_doom_event_t transitions[MAX_EVENTS_PER_TIC];
    event_t event;
    int count;
    int i;

    count = retrodeck_doom_poll_events (transitions, MAX_EVENTS_PER_TIC);

    for (i = 0; i < count; ++i)
    {
        if (transitions[i].key == 0)
        {
            continue;
        }

        // Only mouse events use data3, but the whole struct is copied into
        // the queue, so leave nothing uninitialised in it.
        event.data3 = 0;

        if (transitions[i].pressed)
        {
            event.type = ev_keydown;
            event.data1 = transitions[i].key;
            event.data2 = transitions[i].character;
        }
        else
        {
            event.type = ev_keyup;
            event.data1 = transitions[i].key;

            // data2 is only meaningful for a press.
            event.data2 = 0;
        }

        D_PostEvent (&event);
    }
}

void I_StartTic (void)
{
    I_GetEvent ();

    // A held exit cross, a signal, or a failed present all arrive here.
    //
    // Posting ev_quit would be the obvious move and is wrong: the menu
    // treats it as a click on a window close button and only raises the
    // "really quit" prompt, so a supervised termination would sit waiting
    // for an answer nobody can give. I_Quit runs the engine's exit
    // handlers, and then returns rather than leaving, because this fork
    // compiles out its exit(). The host has to do the leaving.

    if (!leaving && I_RetrodeckQuitRequested ())
    {
        leaving = true;
        I_Quit ();
        retrodeck_doom_exit (0);
    }
}

void I_InitInput (void)
{
    // The host opened the controllers before starting the engine, so there
    // is no device work here. What is left is rebinding the engine to a
    // gamepad, which has to happen after M_LoadDefaults so a stale config
    // file cannot reintroduce keyboard-only bindings. D_DoomLoop calls
    // I_InitGraphics, and therefore this, well after the config is read.

    // A activates menu entries and B leaves them, so the same two buttons
    // mean the same thing in menus and in play.
    key_menu_forward = KEY_FIRE;
    key_menu_back = KEY_USE;

    // Vanilla leaves weapon cycling unbound; X and Y drive it here.
    key_prevweapon = RETRODECK_KEY_PREVWEAPON;
    key_nextweapon = RETRODECK_KEY_NEXTWEAPON;

    joybspeed = RETRODECK_ALWAYS_RUN;
}
