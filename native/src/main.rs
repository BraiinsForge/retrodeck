use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use pbkdf2::pbkdf2_hmac_array;
use retrodeck_native::{
    audio, canvas, chiptune, control_file, controls, fbdev, input, network, polling, process,
    regular_file, state_file, wayland,
};
use sha2::Sha256;
use std::env;
use std::ffi::{CString, OsStr, OsString, c_char, c_int, c_void};
use std::io::Read;
use std::mem;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::ptr;
use subtle::ConstantTimeEq;

type ClObject = *mut c_void;
type ClFixnum = isize;
type EclFixedFunction = unsafe extern "C" fn() -> ClObject;
type EclOneArgumentFunction = unsafe extern "C" fn(ClObject) -> ClObject;
type EclTwoArgumentFunction = unsafe extern "C" fn(ClObject, ClObject) -> ClObject;
type EclThreeArgumentFunction = unsafe extern "C" fn(ClObject, ClObject, ClObject) -> ClObject;
type EclFourArgumentFunction =
    unsafe extern "C" fn(ClObject, ClObject, ClObject, ClObject) -> ClObject;
type EclFiveArgumentFunction =
    unsafe extern "C" fn(ClObject, ClObject, ClObject, ClObject, ClObject) -> ClObject;
type EclTwelveArgumentFunction = unsafe extern "C" fn(
    ClObject,
    ClObject,
    ClObject,
    ClObject,
    ClObject,
    ClObject,
    ClObject,
    ClObject,
    ClObject,
    ClObject,
    ClObject,
    ClObject,
) -> ClObject;

const ECL_NIL: ClObject = 1usize as ClObject;
const FIXNUM_TAG: usize = 3;
const DEFAULT_STARTUP: &str = "/mnt/data/nes-deck/lisp/startup.lisp";
const ABI_VERSION: ClFixnum = 28;
const MAXIMUM_REGULAR_FILE_BYTES: u32 = 4 * 1024 * 1024;

const LOAD_STARTUP: &str = r#"
(handler-case
    (load cl-user::*retrodeck-startup-path* :verbose nil :print nil)
  (error (condition)
    (format *error-output* "retrodeck: failed to load ~A: ~A~%"
            cl-user::*retrodeck-startup-path* condition)
    nil))
"#;

const RUN_MAIN: &str = r#"
(handler-case
    (let* ((package (or (find-package "RETRODECK")
                        (error "The RETRODECK package is missing")))
           (symbol (or (find-symbol "MAIN" package)
                       (error "RETRODECK:MAIN is missing"))))
      (funcall symbol))
  (error (condition)
    (format *error-output* "retrodeck: Lisp orchestrator failed: ~A~%" condition)
    1))
"#;

const RUN_REPL: &str = r#"
(handler-case
    (progn
      (setf *package* (or (find-package "RETRODECK") *package*))
      (format t "retrodeck: interactive REPL (loaded definitions are live)~%")
      (finish-output)
      (si::top-level t)
      0)
  (error (condition)
    (format *error-output* "retrodeck: REPL failed: ~A~%" condition)
    1))
"#;

#[cfg(feature = "lisp-image")]
unsafe extern "C" {
    fn ecl_init_module(block: ClObject, entry: unsafe extern "C" fn(ClObject)) -> ClObject;
    fn init_retrodeck_lisp(block: ClObject);
}

unsafe extern "C" {
    fn cl_boot(argc: c_int, argv: *mut *mut c_char) -> c_int;
    fn cl_shutdown();
    fn ecl_make_symbol(name: *const c_char, package: *const c_char) -> ClObject;
    fn ecl_def_c_function(symbol: ClObject, function: EclFixedFunction, arguments: c_int);
    fn ecl_make_integer(value: ClFixnum) -> ClObject;
    fn ecl_cons(car: ClObject, cdr: ClObject) -> ClObject;
    fn cl_car(value: ClObject) -> ClObject;
    fn cl_cdr(value: ClObject) -> ClObject;
    fn cl_consp(value: ClObject) -> ClObject;
    fn cl_stringp(value: ClObject) -> ClObject;
    fn ecl_make_simple_base_string(value: *const c_char, length: ClFixnum) -> ClObject;
    fn ecl_base_string_pointer_safe(value: ClObject) -> *mut c_char;
    fn ecl_length(value: ClObject) -> ClFixnum;
    fn ecl_find_package(name: *const c_char) -> ClObject;
    fn ecl_make_package(
        name: ClObject,
        nicknames: ClObject,
        use_list: ClObject,
        local_nicknames: ClObject,
    ) -> ClObject;
    fn ecl_defparameter(symbol: ClObject, value: ClObject);
    fn si_string_to_object(arguments: ClFixnum, string: ClObject, ...) -> ClObject;
    fn si_safe_eval(arguments: ClFixnum, form: ClObject, environment: ClObject, ...) -> ClObject;
}

struct Ecl;

impl Ecl {
    fn boot() -> Result<Self, String> {
        let mut arguments = env::args_os()
            .map(|argument| CString::new(argument.as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "process arguments cannot contain NUL".to_owned())?;
        let count = arguments.len();
        let mut pointers = arguments
            .iter_mut()
            .map(|argument| argument.as_ptr().cast_mut())
            .collect::<Vec<_>>();
        pointers.push(ptr::null_mut());

        if unsafe { cl_boot(count as c_int, pointers.as_mut_ptr()) } == 0 {
            return Err("ECL failed to start".to_owned());
        }
        Ok(Self)
    }

    fn register_primitives(&self) -> Result<(), String> {
        let package_name = c_string("RETRODECK.NATIVE")?;
        if unsafe { ecl_find_package(package_name.as_ptr()) } == ECL_NIL {
            let name = unsafe { ecl_make_simple_base_string(package_name.as_ptr(), -1) };
            unsafe { ecl_make_package(name, ECL_NIL, ECL_NIL, ECL_NIL) };
        }

        let abi_name = c_string("ABI-VERSION")?;
        let abi = unsafe { ecl_make_symbol(abi_name.as_ptr(), package_name.as_ptr()) };
        unsafe { ecl_def_c_function(abi, native_abi_version, 0) };

        let play_name = c_string("PLAY-TONES")?;
        let play = unsafe { ecl_make_symbol(play_name.as_ptr(), package_name.as_ptr()) };
        // ECL 26.5.5 defines cl_objectfn_fixed as the C old-style
        // cl_object (*)(), then dispatches the argument count registered here.
        let callback = unsafe {
            mem::transmute::<EclFiveArgumentFunction, EclFixedFunction>(native_play_tones)
        };
        unsafe { ecl_def_c_function(play, callback, 5) };

        let waveform_name = c_string("CANVAS-DRAW-WAVEFORM")?;
        let waveform =
            unsafe { ecl_make_symbol(waveform_name.as_ptr(), package_name.as_ptr()) };
        let callback = unsafe {
            mem::transmute::<EclFourArgumentFunction, EclFixedFunction>(
                native_canvas_draw_waveform,
            )
        };
        unsafe { ecl_def_c_function(waveform, callback, 4) };

        let run_terminal_name = c_string("RUN-TERMINAL")?;
        let run_terminal =
            unsafe { ecl_make_symbol(run_terminal_name.as_ptr(), package_name.as_ptr()) };
        let callback = unsafe {
            mem::transmute::<EclFourArgumentFunction, EclFixedFunction>(native_run_terminal)
        };
        unsafe { ecl_def_c_function(run_terminal, callback, 4) };

        let run_child_name = c_string("RUN-CHILD")?;
        let run_child = unsafe { ecl_make_symbol(run_child_name.as_ptr(), package_name.as_ptr()) };
        let callback = unsafe {
            mem::transmute::<EclFiveArgumentFunction, EclFixedFunction>(native_run_child)
        };
        unsafe { ecl_def_c_function(run_child, callback, 5) };

        for (name, function) in [
            (
                "CANVAS-FILL-RECT",
                native_canvas_fill_rect as EclFiveArgumentFunction,
            ),
            (
                "CANVAS-DRAW-GLYPH",
                native_canvas_draw_glyph as EclFiveArgumentFunction,
            ),
            (
                "CANVAS-DRAW-RASTER",
                native_canvas_draw_raster as EclFiveArgumentFunction,
            ),
        ] {
            let name = c_string(name)?;
            let symbol = unsafe { ecl_make_symbol(name.as_ptr(), package_name.as_ptr()) };
            let callback =
                unsafe { mem::transmute::<EclFiveArgumentFunction, EclFixedFunction>(function) };
            unsafe { ecl_def_c_function(symbol, callback, 5) };
        }

        for (name, function) in [
            (
                "RASTER-LOAD-COVER",
                native_raster_load_cover as EclTwoArgumentFunction,
            ),
            (
                "TEXT-MASK-LOAD",
                native_text_mask_load as EclTwoArgumentFunction,
            ),
            (
                "CANVAS-DRAW-PROJECTED-TEXT",
                native_canvas_draw_projected_text as EclTwoArgumentFunction,
            ),
            (
                "PLAY-TONE-SEQUENCE",
                native_play_tone_sequence as EclTwoArgumentFunction,
            ),
            ("INPUT-POLL", native_input_poll as EclTwoArgumentFunction),
            ("RUN-HELPER", native_run_helper as EclTwoArgumentFunction),
            (
                "WRITE-CONTROL-FILE",
                native_write_control_file as EclTwoArgumentFunction,
            ),
            (
                "WRITE-STATE-FILE",
                native_write_state_file as EclTwoArgumentFunction,
            ),
        ] {
            let name = c_string(name)?;
            let symbol = unsafe { ecl_make_symbol(name.as_ptr(), package_name.as_ptr()) };
            let callback =
                unsafe { mem::transmute::<EclTwoArgumentFunction, EclFixedFunction>(function) };
            unsafe { ecl_def_c_function(symbol, callback, 2) };
        }

        for (name, function) in [
            (
                "RASTER-LOAD-PNG",
                native_raster_load_png as EclThreeArgumentFunction,
            ),
            (
                "READ-REGULAR-FILE",
                native_read_regular_file as EclThreeArgumentFunction,
            ),
        ] {
            let name = c_string(name)?;
            let symbol = unsafe { ecl_make_symbol(name.as_ptr(), package_name.as_ptr()) };
            let callback =
                unsafe { mem::transmute::<EclThreeArgumentFunction, EclFixedFunction>(function) };
            unsafe { ecl_def_c_function(symbol, callback, 3) };
        }

        let name = c_string("CANVAS-CONFIGURE-PROJECTION")?;
        let symbol = unsafe { ecl_make_symbol(name.as_ptr(), package_name.as_ptr()) };
        let callback = unsafe {
            mem::transmute::<EclTwelveArgumentFunction, EclFixedFunction>(
                native_canvas_configure_projection,
            )
        };
        unsafe { ecl_def_c_function(symbol, callback, 12) };

        for (name, function) in [
            ("AUDIO-ACTIVE-P", native_audio_active as EclFixedFunction),
            ("CHIPTUNE-STEP", native_chiptune_step as EclFixedFunction),
            (
                "CHIPTUNE-REWIND",
                native_chiptune_rewind as EclFixedFunction,
            ),
            ("CHIPTUNE-CLOSE", native_chiptune_close as EclFixedFunction),
            (
                "CHIPTUNE-AUDIO-CLOSE",
                native_chiptune_audio_close as EclFixedFunction,
            ),
            (
                "PROCESS-SHUTDOWN-P",
                native_process_shutdown as EclFixedFunction,
            ),
            (
                "PROGRAM-ARGUMENTS",
                native_program_arguments as EclFixedFunction,
            ),
            (
                "CANVAS-RGB565-HASH-WORDS",
                native_canvas_rgb565_hash_words as EclFixedFunction,
            ),
            (
                "MONOTONIC-NANOSECONDS-WORDS",
                native_monotonic_nanoseconds_words as EclFixedFunction,
            ),
            ("STOP-AUDIO", native_stop_audio as EclFixedFunction),
            ("FINISH-AUDIO", native_finish_audio as EclFixedFunction),
            ("RASTER-CLEAR", native_raster_clear as EclFixedFunction),
            (
                "TEXT-MASK-CLEAR",
                native_text_mask_clear as EclFixedFunction,
            ),
            (
                "EVDEV-CONTROLS-SCAN",
                native_evdev_controls_scan as EclFixedFunction,
            ),
            (
                "EVDEV-GAMEPADS-SCAN",
                native_evdev_gamepads_scan as EclFixedFunction,
            ),
            (
                "EVDEV-CONTROLS-CLOSE",
                native_evdev_controls_close as EclFixedFunction,
            ),
            (
                "EVDEV-NEXT-CONTROL",
                native_evdev_next_control as EclFixedFunction,
            ),
            (
                "EVDEV-TOUCH-OPEN",
                native_evdev_touch_open as EclFixedFunction,
            ),
            (
                "EVDEV-TOUCH-CLOSE",
                native_evdev_touch_close as EclFixedFunction,
            ),
            (
                "EVDEV-NEXT-TOUCH",
                native_evdev_next_touch as EclFixedFunction,
            ),
            (
                "FBDEV-PRESENT-CANVAS",
                native_fbdev_present_canvas as EclFixedFunction,
            ),
            ("FBDEV-OPEN", native_fbdev_open as EclFixedFunction),
            ("FBDEV-CLOSE", native_fbdev_close as EclFixedFunction),
            ("FBDEV-SIZE", native_fbdev_size as EclFixedFunction),
            (
                "WAYLAND-PRESENT-CANVAS",
                native_wayland_present_canvas as EclFixedFunction,
            ),
            (
                "WAYLAND-OPEN-WIDGET",
                native_wayland_open_widget as EclFixedFunction,
            ),
            ("WAYLAND-CLOSE", native_wayland_close as EclFixedFunction),
            (
                "WAYLAND-NEXT-TOUCH",
                native_wayland_next_touch as EclFixedFunction,
            ),
            ("WAYLAND-SIZE", native_wayland_size as EclFixedFunction),
            (
                "WAYLAND-SHUTDOWN-P",
                native_wayland_shutdown as EclFixedFunction,
            ),
        ] {
            let name = c_string(name)?;
            let symbol = unsafe { ecl_make_symbol(name.as_ptr(), package_name.as_ptr()) };
            unsafe { ecl_def_c_function(symbol, function, 0) };
        }

        for (name, function) in [
            (
                "CHIPTUNE-OPEN",
                native_chiptune_open as EclOneArgumentFunction,
            ),
            (
                "CHIPTUNE-START-TRACK",
                native_chiptune_start_track as EclOneArgumentFunction,
            ),
            (
                "LIST-DIRECTORY",
                native_list_directory as EclOneArgumentFunction,
            ),
            (
                "CHIPTUNE-AUDIO-OPEN",
                native_chiptune_audio_open as EclOneArgumentFunction,
            ),
            (
                "CHIPTUNE-AUDIO-WRITE",
                native_chiptune_audio_write as EclOneArgumentFunction,
            ),
            (
                "CANVAS-CLEAR",
                native_canvas_clear as EclOneArgumentFunction,
            ),
            (
                "EVDEV-CONTROLS-DISPATCH",
                native_evdev_controls_dispatch as EclOneArgumentFunction,
            ),
            (
                "EVDEV-TOUCH-DISPATCH",
                native_evdev_touch_dispatch as EclOneArgumentFunction,
            ),
            (
                "FBDEV-PRESENT-SOLID",
                native_fbdev_present_solid as EclOneArgumentFunction,
            ),
            (
                "WAYLAND-PRESENT-SOLID",
                native_wayland_present_solid as EclOneArgumentFunction,
            ),
            (
                "WAYLAND-DISPATCH",
                native_wayland_dispatch as EclOneArgumentFunction,
            ),
            (
                "WAYLAND-OPEN-WIDGET-AT",
                native_wayland_open_widget_at as EclOneArgumentFunction,
            ),
            (
                "WAYLAND-OPEN-GAMEPLAY-AT",
                native_wayland_open_gameplay_at as EclOneArgumentFunction,
            ),
            (
                "NETWORK-STATUS",
                native_network_status as EclOneArgumentFunction,
            ),
            (
                "READ-CONTROL-FILE",
                native_read_control_file as EclOneArgumentFunction,
            ),
            (
                "READ-STATE-FILE",
                native_read_state_file as EclOneArgumentFunction,
            ),
        ] {
            let name = c_string(name)?;
            let symbol = unsafe { ecl_make_symbol(name.as_ptr(), package_name.as_ptr()) };
            let callback =
                unsafe { mem::transmute::<EclOneArgumentFunction, EclFixedFunction>(function) };
            unsafe { ecl_def_c_function(symbol, callback, 1) };
        }
        Ok(())
    }

    fn load(&self, path: &Path) -> Result<(), String> {
        let path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| "the Lisp startup path cannot contain NUL".to_owned())?;
        let path_value = unsafe { ecl_make_simple_base_string(path.as_ptr(), -1) };
        let variable_name = c_string("*RETRODECK-STARTUP-PATH*")?;
        let package_name = c_string("CL-USER")?;
        let variable = unsafe { ecl_make_symbol(variable_name.as_ptr(), package_name.as_ptr()) };
        unsafe { ecl_defparameter(variable, path_value) };

        if self.evaluate(LOAD_STARTUP) == ECL_NIL {
            return Err("Common Lisp startup failed".to_owned());
        }
        Ok(())
    }

    #[cfg(feature = "lisp-image")]
    fn initialize_image(&self) {
        unsafe { ecl_init_module(ptr::null_mut(), init_retrodeck_lisp) };
    }

    fn run(&self) -> Result<u8, String> {
        decode_exit_code(self.evaluate(RUN_MAIN))
    }

    fn repl(&self) -> Result<u8, String> {
        decode_exit_code(self.evaluate(RUN_REPL))
    }

    fn evaluate(&self, source: &str) -> ClObject {
        let source = CString::new(source).expect("embedded Lisp contains no NUL");
        let string = unsafe { ecl_make_simple_base_string(source.as_ptr(), -1) };
        let form = unsafe { si_string_to_object(1, string) };
        unsafe { si_safe_eval(3, form, ECL_NIL, ECL_NIL) }
    }
}

impl Drop for Ecl {
    fn drop(&mut self) {
        fbdev::close();
        input::close_touch();
        wayland::close();
        let _ = audio::close_chiptune_pcm();
        audio::stop();
        let _ = chiptune::close();
        unsafe { cl_shutdown() };
    }
}

unsafe extern "C" fn native_abi_version() -> ClObject {
    unsafe { ecl_make_integer(ABI_VERSION) }
}

fn chiptune_metadata_list(result: Result<chiptune::Metadata, String>) -> ClObject {
    match result {
        Ok(metadata) => make_object_list(&[
            make_base_string(&metadata.title, "chiptune title"),
            make_base_string(&metadata.artist, "chiptune artist"),
            make_base_string(&metadata.author, "chiptune author"),
            make_base_string(&metadata.system, "chiptune system"),
            unsafe { ecl_make_integer(metadata.length_ms as ClFixnum) },
            unsafe { ecl_make_integer(metadata.track_count as ClFixnum) },
        ]),
        Err(error) => {
            eprintln!("retrodeck: {error}");
            ECL_NIL
        }
    }
}

unsafe extern "C" fn native_chiptune_open(path: ClObject) -> ClObject {
    chiptune_metadata_list(decode_path(path, "chiptune path").and_then(|path| chiptune::open(&path)))
}

unsafe extern "C" fn native_chiptune_start_track(track: ClObject) -> ClObject {
    chiptune_metadata_list(decode_i32(track, "chiptune track").and_then(chiptune::start_track))
}

unsafe extern "C" fn native_chiptune_step() -> ClObject {
    match chiptune::step() {
        Ok(snapshot) => {
            let pcm = snapshot
                .samples
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect::<Vec<_>>();
            make_object_list(&[
                make_base_string(&pcm, "chiptune PCM"),
                unsafe { ecl_make_integer(boolean_fixnum(snapshot.ended)) },
                unsafe { ecl_make_integer(snapshot.frames as ClFixnum) },
                unsafe { ecl_make_integer(snapshot.position_ms as ClFixnum) },
            ])
        }
        Err(error) => {
            eprintln!("retrodeck: {error}");
            ECL_NIL
        }
    }
}

unsafe extern "C" fn native_chiptune_rewind() -> ClObject {
    native_status(chiptune::rewind())
}

unsafe extern "C" fn native_chiptune_close() -> ClObject {
    native_status(chiptune::close())
}

unsafe extern "C" fn native_chiptune_audio_open(volume: ClObject) -> ClObject {
    native_status(decode_i32(volume, "chiptune volume").and_then(audio::open_chiptune_pcm))
}

unsafe extern "C" fn native_chiptune_audio_write(pcm: ClObject) -> ClObject {
    let result =
        decode_base_string(pcm, "chiptune PCM").and_then(|pcm| audio::write_chiptune_pcm(&pcm));
    let status = match result {
        Ok(audio::PcmWriteOutcome::Queued) => 1,
        Ok(audio::PcmWriteOutcome::Busy) => 0,
        Err(error) => {
            eprintln!("retrodeck: {error}");
            0
        }
    };
    unsafe { ecl_make_integer(status) }
}

unsafe extern "C" fn native_chiptune_audio_close() -> ClObject {
    native_status(audio::close_chiptune_pcm())
}

unsafe extern "C" fn native_run_helper(executable: ClObject, input: ClObject) -> ClObject {
    let result = (|| {
        let executable = decode_path(executable, "helper executable")?;
        let input = decode_base_string(input, "helper input")?;
        Ok(process::run_helper(&executable, &input))
    })();
    let result = result.unwrap_or_else(|error| process::HelperResult {
        phase: process::HelperPhase::Start,
        exit_code: None,
        signal: None,
        error: Some(error),
    });
    let phase = match result.phase {
        process::HelperPhase::Complete => 0,
        process::HelperPhase::Start => 1,
        process::HelperPhase::Input => 2,
        process::HelperPhase::Wait => 3,
    };
    let error = result.error.as_deref().map_or(ECL_NIL, |error| {
        make_base_string(error.as_bytes(), "helper error")
    });
    make_object_list(&[
        unsafe { ecl_make_integer(phase) },
        unsafe { ecl_make_integer(result.exit_code.map_or(-1, |value| value as ClFixnum)) },
        unsafe { ecl_make_integer(result.signal.map_or(-1, |value| value as ClFixnum)) },
        error,
    ])
}

unsafe extern "C" fn native_run_terminal(
    executable: ClObject,
    keymap: ClObject,
    mode: ClObject,
    label: ClObject,
) -> ClObject {
    let result = (|| {
        let executable = decode_path(executable, "terminal executable")?;
        let keymap = decode_base_string(keymap, "terminal keymap")?;
        let mode = decode_base_string(mode, "terminal mode")?;
        let label = String::from_utf8(decode_base_string(label, "terminal label")?)
            .map_err(|_| "terminal label is not UTF-8".to_owned())?;
        Ok(process::run_terminal(
            &executable,
            OsStr::from_bytes(&keymap),
            OsStr::from_bytes(&mode),
            &label,
        ))
    })();
    make_child_result(result.unwrap_or_else(child_decode_error))
}

unsafe extern "C" fn native_run_child(
    executable: ClObject,
    arguments: ClObject,
    environment: ClObject,
    label: ClObject,
    touch_supervision: ClObject,
) -> ClObject {
    let result = (|| {
        let executable = decode_path(executable, "child executable")?;
        let arguments = decode_os_string_list(arguments, "child arguments")?;
        let environment = decode_environment(environment)?;
        let label = String::from_utf8(decode_base_string(label, "child label")?)
            .map_err(|_| "child label is not UTF-8".to_owned())?;
        let touch_supervision = decode_boolean_fixnum(touch_supervision, "child touch flag")?;
        Ok(process::run_child(
            &executable,
            &arguments,
            &environment,
            &label,
            touch_supervision,
            false,
        ))
    })();
    make_child_result(result.unwrap_or_else(child_decode_error))
}

unsafe extern "C" fn native_play_tones(
    first_frequency: ClObject,
    first_duration_ms: ClObject,
    second_frequency: ClObject,
    second_duration_ms: ClObject,
    volume_percent: ClObject,
) -> ClObject {
    native_play_outcome((|| {
        audio::play_tones(
            decode_i32(first_frequency, "first tone frequency")?,
            decode_i32(first_duration_ms, "first tone duration")?,
            decode_i32(second_frequency, "second tone frequency")?,
            decode_i32(second_duration_ms, "second tone duration")?,
            decode_i32(volume_percent, "menu sound volume")?,
        )
    })())
}

unsafe extern "C" fn native_play_tone_sequence(
    tones: ClObject,
    volume_percent: ClObject,
) -> ClObject {
    native_play_outcome((|| {
        audio::play_tone_sequence(
            &decode_tone_sequence(tones)?,
            decode_i32(volume_percent, "sound volume")?,
        )
    })())
}

fn native_play_outcome(result: Result<audio::PlayOutcome, String>) -> ClObject {
    let status = match result {
        Ok(audio::PlayOutcome::Started) => 1,
        Ok(audio::PlayOutcome::Busy) => 2,
        Err(error) => {
            eprintln!("retrodeck: {error}");
            0
        }
    };
    unsafe { ecl_make_integer(status) }
}

unsafe extern "C" fn native_audio_active() -> ClObject {
    unsafe { ecl_make_integer(if audio::active() { 1 } else { 0 }) }
}

unsafe extern "C" fn native_process_shutdown() -> ClObject {
    unsafe { ecl_make_integer(boolean_fixnum(process::shutdown_requested())) }
}

unsafe extern "C" fn native_stop_audio() -> ClObject {
    audio::stop();
    unsafe { ecl_make_integer(0) }
}

unsafe extern "C" fn native_finish_audio() -> ClObject {
    audio::finish();
    unsafe { ecl_make_integer(0) }
}

unsafe extern "C" fn native_canvas_rgb565_hash_words() -> ClObject {
    make_unsigned_64_words(canvas::rgb565_hash())
}

unsafe extern "C" fn native_monotonic_nanoseconds_words() -> ClObject {
    make_unsigned_64_words(polling::monotonic_nanoseconds())
}

unsafe extern "C" fn native_canvas_clear(color: ClObject) -> ClObject {
    let result = (|| {
        canvas::clear(decode_color(color, "canvas clear color")?);
        Ok(())
    })();
    native_status(result)
}

unsafe extern "C" fn native_canvas_draw_waveform(
    pcm: ClObject,
    background: ClObject,
    muted: ClObject,
    orange: ClObject,
) -> ClObject {
    let result = (|| {
        let pcm = decode_base_string(pcm, "waveform PCM")?;
        canvas::draw_waveform(
            &pcm,
            decode_color(background, "waveform background")?,
            decode_color(muted, "waveform midline")?,
            decode_color(orange, "waveform bars")?,
        )
    })();
    native_status(result)
}

unsafe extern "C" fn native_canvas_fill_rect(
    x: ClObject,
    y: ClObject,
    width: ClObject,
    height: ClObject,
    color: ClObject,
) -> ClObject {
    let result = (|| {
        canvas::fill_rect(
            decode_i32(x, "canvas rectangle x")?,
            decode_i32(y, "canvas rectangle y")?,
            decode_u32(width, "canvas rectangle width")?,
            decode_u32(height, "canvas rectangle height")?,
            decode_color(color, "canvas rectangle color")?,
        );
        Ok(())
    })();
    native_status(result)
}

unsafe extern "C" fn native_canvas_draw_glyph(
    x: ClObject,
    y: ClObject,
    character: ClObject,
    scale: ClObject,
    color: ClObject,
) -> ClObject {
    let result = (|| {
        let character = u8::try_from(decode_u32(character, "canvas glyph character")?)
            .map_err(|_| "canvas glyph character is out of range".to_owned())?;
        let scale = decode_u32(scale, "canvas glyph scale")?;
        if scale == 0 {
            return Err("canvas glyph scale must be positive".to_owned());
        }
        canvas::draw_glyph(
            decode_i32(x, "canvas glyph x")?,
            decode_i32(y, "canvas glyph y")?,
            character,
            scale,
            decode_color(color, "canvas glyph color")?,
        );
        Ok(())
    })();
    native_status(result)
}

unsafe extern "C" fn native_text_mask_load(text: ClObject, scale: ClObject) -> ClObject {
    let result = (|| {
        canvas::load_text_mask(
            &decode_base_string(text, "projected text")?,
            decode_u32(scale, "projected text scale")?,
        )
    })();
    native_handle(result)
}

#[allow(clippy::too_many_arguments)]
unsafe extern "C" fn native_canvas_configure_projection(
    elapsed_ms: ClObject,
    speed_numerator: ClObject,
    speed_denominator: ClObject,
    cycle: ClObject,
    camera_distance: ClObject,
    maximum_depth: ClObject,
    horizon_y: ClObject,
    clip_top: ClObject,
    fade_invisible_y: ClObject,
    fade_opaque_y: ClObject,
    bottom_y: ClObject,
    color: ClObject,
) -> ClObject {
    let result = (|| {
        canvas::configure_projection(
            decode_i64_hex(elapsed_ms, "projection elapsed time")?,
            decode_u32(speed_numerator, "projection speed numerator")?,
            decode_u32(speed_denominator, "projection speed denominator")?,
            decode_u32(cycle, "projection cycle")?,
            decode_u32(camera_distance, "projection camera distance")?,
            decode_u32(maximum_depth, "projection maximum depth")?,
            decode_i32(horizon_y, "projection horizon")?,
            decode_i32(clip_top, "projection clip top")?,
            decode_i32(fade_invisible_y, "projection invisible fade")?,
            decode_i32(fade_opaque_y, "projection opaque fade")?,
            decode_i32(bottom_y, "projection bottom")?,
            decode_color(color, "projection color")?,
        )
    })();
    native_status(result)
}

unsafe extern "C" fn native_canvas_draw_projected_text(
    handle: ClObject,
    source_y: ClObject,
) -> ClObject {
    let result = (|| {
        canvas::draw_projected_text(
            decode_u32(handle, "projected text mask handle")?,
            decode_i32(source_y, "projected text source y")?,
        )
    })();
    native_status(result)
}

unsafe extern "C" fn native_text_mask_clear() -> ClObject {
    canvas::clear_text_masks();
    unsafe { ecl_make_integer(1) }
}

unsafe extern "C" fn native_raster_clear() -> ClObject {
    canvas::clear_rasters();
    unsafe { ecl_make_integer(1) }
}

unsafe extern "C" fn native_raster_load_cover(path: ClObject, background: ClObject) -> ClObject {
    let result = (|| {
        canvas::load_cover_raster(
            &decode_path(path, "cover raster path")?,
            decode_color(background, "cover raster background")?,
        )
    })();
    native_handle(result)
}

unsafe extern "C" fn native_raster_load_png(
    path: ClObject,
    width: ClObject,
    height: ClObject,
) -> ClObject {
    let result = (|| {
        canvas::load_png_raster(
            &decode_path(path, "PNG raster path")?,
            decode_u32(width, "PNG raster width")?,
            decode_u32(height, "PNG raster height")?,
        )
    })();
    native_handle(result)
}

unsafe extern "C" fn native_read_regular_file(
    path: ClObject,
    minimum_bytes: ClObject,
    maximum_bytes: ClObject,
) -> ClObject {
    let result = (|| {
        let minimum_bytes = decode_u32(minimum_bytes, "minimum regular file bytes")?;
        let maximum_bytes = decode_u32(maximum_bytes, "maximum regular file bytes")?;
        if minimum_bytes > maximum_bytes || maximum_bytes > MAXIMUM_REGULAR_FILE_BYTES {
            return Err(format!(
                "regular file byte bounds must not exceed {MAXIMUM_REGULAR_FILE_BYTES}"
            ));
        }
        regular_file::read_regular(
            &decode_path(path, "regular file path")?,
            u64::from(minimum_bytes),
            u64::from(maximum_bytes),
            "file",
        )
    })();
    native_optional_string(result)
}

unsafe extern "C" fn native_program_arguments() -> ClObject {
    let items = env::args_os()
        .map(|argument| make_base_string(argument.as_bytes(), "program argument"))
        .collect::<Vec<_>>();
    make_object_list(&items)
}

unsafe extern "C" fn native_list_directory(path: ClObject) -> ClObject {
    let result = decode_path(path, "directory path")
        .and_then(|path| regular_file::list_directory(&path));
    match result {
        Ok(entries) => {
            let items = entries
                .iter()
                .map(|entry| {
                    make_object_list(&[
                        make_base_string(&entry.name, "directory entry name"),
                        unsafe { ecl_make_integer(entry.kind as ClFixnum) },
                        unsafe {
                            ecl_make_integer(ClFixnum::try_from(entry.size).unwrap_or(ClFixnum::MAX))
                        },
                    ])
                })
                .collect::<Vec<_>>();
            make_object_list(&items)
        }
        Err(error) => {
            eprintln!("retrodeck: {error}");
            ECL_NIL
        }
    }
}

unsafe extern "C" fn native_read_control_file(path: ClObject) -> ClObject {
    let result: Result<Option<Vec<u8>>, String> = (|| {
        Ok(Some(control_file::read(&decode_path(
            path,
            "control file path",
        )?)?))
    })();
    native_optional_string(result)
}

unsafe extern "C" fn native_write_control_file(path: ClObject, value: ClObject) -> ClObject {
    let result = (|| {
        control_file::write(
            &decode_path(path, "control file path")?,
            &decode_base_string(value, "control file value")?,
        )
    })();
    native_status(result)
}

unsafe extern "C" fn native_read_state_file(path: ClObject) -> ClObject {
    let result = (|| state_file::read(&decode_path(path, "state file path")?))();
    match result {
        Ok(state_file::StateRead::Missing) => make_fixnum_list(&[0]),
        Ok(state_file::StateRead::Value(value)) => make_object_list(&[
            unsafe { ecl_make_integer(1) },
            make_base_string(&value, "state file"),
        ]),
        Err(error) => {
            eprintln!("retrodeck: {error}");
            ECL_NIL
        }
    }
}

unsafe extern "C" fn native_write_state_file(path: ClObject, value: ClObject) -> ClObject {
    let result = (|| {
        state_file::write(
            &decode_path(path, "state file path")?,
            &decode_base_string(value, "state file value")?,
        )
    })();
    native_status(result)
}

unsafe extern "C" fn native_network_status(path: ClObject) -> ClObject {
    let result: Result<network::NetworkStatus, String> = (|| {
        Ok(network::read_network_status(&decode_path(
            path,
            "network selector status path",
        )?))
    })();
    match result {
        Ok(status) => make_object_list(&[
            make_base_string(status.ssid.as_bytes(), "network SSID"),
            make_base_string(status.wlan_ipv4.as_bytes(), "wlan0 address"),
            make_base_string(status.wireguard_ipv4.as_bytes(), "wg0 address"),
            make_base_string(status.selector.as_bytes(), "network selector status"),
        ]),
        Err(error) => {
            eprintln!("retrodeck: {error}");
            ECL_NIL
        }
    }
}

unsafe extern "C" fn native_canvas_draw_raster(
    handle: ClObject,
    x: ClObject,
    y: ClObject,
    width: ClObject,
    height: ClObject,
) -> ClObject {
    let result = (|| {
        canvas::draw_raster(
            decode_u32(handle, "canvas raster handle")?,
            decode_i32(x, "canvas raster x")?,
            decode_i32(y, "canvas raster y")?,
            decode_u32(width, "canvas raster width")?,
            decode_u32(height, "canvas raster height")?,
        )
    })();
    native_status(result)
}

unsafe extern "C" fn native_input_poll(
    wayland_backend: ClObject,
    timeout_ms: ClObject,
) -> ClObject {
    let result = (|| {
        let wayland_backend = decode_u32(wayland_backend, "input poll backend")?;
        if wayland_backend > 1 {
            return Err("input poll backend must be zero or one".to_owned());
        }
        polling::dispatch(
            wayland_backend == 1,
            decode_u32(timeout_ms, "input poll timeout")?,
        )
    })();
    match result {
        Ok(dispatch) => make_fixnum_list(&[
            boolean_fixnum(dispatch.ready),
            dispatch.control_count as ClFixnum,
            dispatch.touch_count as ClFixnum,
            boolean_fixnum(dispatch.touch_lost),
            boolean_fixnum(dispatch.rescan),
            boolean_fixnum(dispatch.shutdown),
            boolean_fixnum(dispatch.refresh),
        ]),
        Err(error) => {
            eprintln!("retrodeck: {error}");
            ECL_NIL
        }
    }
}

unsafe extern "C" fn native_evdev_controls_scan() -> ClObject {
    match controls::scan() {
        Ok((gamepads, keyboards)) => {
            make_fixnum_list(&[gamepads as ClFixnum, keyboards as ClFixnum])
        }
        Err(error) => {
            eprintln!("retrodeck: {error}");
            ECL_NIL
        }
    }
}

unsafe extern "C" fn native_evdev_gamepads_scan() -> ClObject {
    match controls::scan_gamepads() {
        Ok(gamepads) => {
            make_object_list(&[unsafe { ecl_make_integer(gamepads as ClFixnum) }, ECL_NIL])
        }
        Err(error) => make_object_list(&[
            unsafe { ecl_make_integer(-1) },
            make_base_string(error.as_bytes(), "gamepad scan error"),
        ]),
    }
}

unsafe extern "C" fn native_evdev_controls_close() -> ClObject {
    controls::close();
    unsafe { ecl_make_integer(0) }
}

unsafe extern "C" fn native_evdev_controls_dispatch(timeout_ms: ClObject) -> ClObject {
    let result = (|| {
        let timeout_ms = decode_u32(timeout_ms, "evdev controls dispatch timeout")?;
        controls::dispatch(timeout_ms)
    })();
    match result {
        Ok((count, rescan)) => make_fixnum_list(&[count as ClFixnum, boolean_fixnum(rescan)]),
        Err(error) => {
            eprintln!("retrodeck: {error}");
            ECL_NIL
        }
    }
}

unsafe extern "C" fn native_evdev_next_control() -> ClObject {
    let Some(report) = controls::next_report() else {
        return ECL_NIL;
    };
    make_fixnum_list(&[
        report.kind as ClFixnum,
        report.value as ClFixnum,
        report.flags as ClFixnum,
    ])
}

unsafe extern "C" fn native_evdev_touch_open() -> ClObject {
    native_status(input::open_touch())
}

unsafe extern "C" fn native_evdev_touch_close() -> ClObject {
    input::close_touch();
    unsafe { ecl_make_integer(0) }
}

unsafe extern "C" fn native_evdev_touch_dispatch(timeout_ms: ClObject) -> ClObject {
    let result = (|| {
        let timeout_ms = decode_u32(timeout_ms, "evdev touch dispatch timeout")?;
        input::dispatch_touch(timeout_ms)
    })();
    native_count(result, "evdev touch dispatch count")
}

unsafe extern "C" fn native_evdev_next_touch() -> ClObject {
    let Some(report) = input::next_touch() else {
        return ECL_NIL;
    };
    make_touch_report(report)
}

unsafe extern "C" fn native_fbdev_open() -> ClObject {
    native_status(fbdev::open())
}

unsafe extern "C" fn native_fbdev_close() -> ClObject {
    fbdev::close();
    unsafe { ecl_make_integer(0) }
}

unsafe extern "C" fn native_fbdev_present_canvas() -> ClObject {
    native_status(canvas::with_pixels(fbdev::present_rgba))
}

unsafe extern "C" fn native_fbdev_present_solid(color: ClObject) -> ClObject {
    let result = (|| fbdev::present_solid(decode_color(color, "fbdev solid color")?))();
    native_status(result)
}

unsafe extern "C" fn native_fbdev_size() -> ClObject {
    let Some((width, height)) = fbdev::size() else {
        return ECL_NIL;
    };
    make_fixnum_list(&[width as ClFixnum, height as ClFixnum])
}

unsafe extern "C" fn native_wayland_open_widget() -> ClObject {
    native_status(wayland::open_widget())
}

unsafe extern "C" fn native_wayland_open_widget_at(display: ClObject) -> ClObject {
    let result = (|| {
        let display = decode_path(display, "Wayland display")?;
        wayland::open_widget_at(&display)
    })();
    native_status(result)
}

unsafe extern "C" fn native_wayland_open_gameplay_at(display: ClObject) -> ClObject {
    let result = (|| {
        let display = decode_path(display, "Wayland display")?;
        wayland::open_gameplay_at(&display)
    })();
    native_status(result)
}

unsafe extern "C" fn native_wayland_close() -> ClObject {
    wayland::close();
    unsafe { ecl_make_integer(0) }
}

unsafe extern "C" fn native_wayland_present_canvas() -> ClObject {
    native_status(canvas::with_pixels(wayland::present_rgba))
}

unsafe extern "C" fn native_wayland_present_solid(color: ClObject) -> ClObject {
    let result = (|| wayland::present_solid(decode_color(color, "Wayland solid color")?))();
    native_status(result)
}

unsafe extern "C" fn native_wayland_dispatch(timeout_ms: ClObject) -> ClObject {
    let result = (|| {
        let timeout_ms = decode_u32(timeout_ms, "Wayland dispatch timeout")?;
        wayland::dispatch(timeout_ms)
    })();
    native_count(result, "Wayland dispatch count")
}

unsafe extern "C" fn native_wayland_next_touch() -> ClObject {
    let Some(report) = wayland::next_touch() else {
        return ECL_NIL;
    };
    make_touch_report(report)
}

unsafe extern "C" fn native_wayland_size() -> ClObject {
    let Some((width, height)) = wayland::size() else {
        return ECL_NIL;
    };
    make_fixnum_list(&[width as ClFixnum, height as ClFixnum])
}

unsafe extern "C" fn native_wayland_shutdown() -> ClObject {
    unsafe { ecl_make_integer(boolean_fixnum(wayland::shutdown_requested())) }
}

fn native_count(result: Result<usize, String>, name: &str) -> ClObject {
    let value = match result {
        Ok(count) => match ClFixnum::try_from(count) {
            Ok(count) => count,
            Err(_) => {
                eprintln!("retrodeck: {name} is out of range");
                -1
            }
        },
        Err(error) => {
            eprintln!("retrodeck: {error}");
            -1
        }
    };
    unsafe { ecl_make_integer(value) }
}

fn make_touch_report(report: input::TouchReport) -> ClObject {
    make_fixnum_list(&[
        report.x as ClFixnum,
        report.y as ClFixnum,
        boolean_fixnum(report.down),
        boolean_fixnum(report.pressed),
        boolean_fixnum(report.released),
    ])
}

fn native_status(result: Result<(), String>) -> ClObject {
    let status = match result {
        Ok(()) => 1,
        Err(error) => {
            eprintln!("retrodeck: {error}");
            0
        }
    };
    unsafe { ecl_make_integer(status) }
}

fn native_handle(result: Result<u32, String>) -> ClObject {
    let handle = match result {
        Ok(handle) if u64::from(handle) <= (ClFixnum::MAX as u64 >> 2) => handle as ClFixnum,
        Ok(_) => {
            eprintln!("retrodeck: native raster handle is out of ECL fixnum range");
            0
        }
        Err(error) => {
            eprintln!("retrodeck: {error}");
            0
        }
    };
    unsafe { ecl_make_integer(handle) }
}

fn native_optional_string(result: Result<Option<Vec<u8>>, String>) -> ClObject {
    match result {
        Ok(Some(value)) => make_base_string(&value, "regular file"),
        Ok(None) => ECL_NIL,
        Err(error) => {
            eprintln!("retrodeck: {error}");
            ECL_NIL
        }
    }
}

fn child_decode_error(error: String) -> process::ChildResult {
    process::ChildResult {
        error: Some(error),
        ..process::ChildResult::default()
    }
}

fn make_child_result(result: process::ChildResult) -> ClObject {
    let error = result.error.as_deref().map_or(ECL_NIL, |error| {
        make_base_string(error.as_bytes(), "child error")
    });
    make_object_list(&[
        unsafe { ecl_make_integer(boolean_fixnum(result.started)) },
        unsafe { ecl_make_integer(boolean_fixnum(result.exited_for_touch)) },
        unsafe { ecl_make_integer(result.exit_code.map_or(-1, |value| value as ClFixnum)) },
        unsafe { ecl_make_integer(result.signal.map_or(-1, |value| value as ClFixnum)) },
        error,
        unsafe { ecl_make_integer(boolean_fixnum(result.shutdown_requested)) },
    ])
}

fn make_base_string(value: &[u8], name: &str) -> ClObject {
    let Ok(length) = ClFixnum::try_from(value.len()) else {
        eprintln!("retrodeck: {name} is too large for ECL");
        return ECL_NIL;
    };
    unsafe { ecl_make_simple_base_string(value.as_ptr().cast(), length) }
}

fn make_object_list(values: &[ClObject]) -> ClObject {
    let mut list = ECL_NIL;
    for value in values.iter().rev() {
        unsafe {
            list = ecl_cons(*value, list);
        }
    }
    list
}

fn make_fixnum_list(values: &[ClFixnum]) -> ClObject {
    let mut list = ECL_NIL;
    for value in values.iter().rev() {
        unsafe {
            list = ecl_cons(ecl_make_integer(*value), list);
        }
    }
    list
}

fn make_unsigned_64_words(value: u64) -> ClObject {
    make_fixnum_list(&[
        ((value >> 48) & 0xffff) as ClFixnum,
        ((value >> 32) & 0xffff) as ClFixnum,
        ((value >> 16) & 0xffff) as ClFixnum,
        (value & 0xffff) as ClFixnum,
    ])
}

fn boolean_fixnum(value: bool) -> ClFixnum {
    if value { 1 } else { 0 }
}

fn c_string(value: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| "an internal ECL name contains NUL".to_owned())
}

fn decode_base_string(object: ClObject, name: &str) -> Result<Vec<u8>, String> {
    let length = unsafe { ecl_length(object) };
    if length < 0 {
        return Err(format!("{name} has an invalid length"));
    }
    let pointer = unsafe { ecl_base_string_pointer_safe(object) }.cast::<u8>();
    if pointer.is_null() {
        return Err(format!("{name} is unavailable"));
    }
    Ok(unsafe { std::slice::from_raw_parts(pointer, length as usize) }.to_vec())
}

fn decode_path(object: ClObject, name: &str) -> Result<PathBuf, String> {
    let bytes = decode_base_string(object, name)?;
    if bytes.contains(&0) {
        return Err(format!("{name} cannot contain NUL"));
    }
    Ok(PathBuf::from(OsStr::from_bytes(&bytes)))
}

fn decode_object_list(
    object: ClObject,
    name: &str,
    maximum: usize,
) -> Result<Vec<ClObject>, String> {
    let mut values = Vec::new();
    let mut seen = Vec::new();
    let mut cursor = object;
    while cursor != ECL_NIL {
        if unsafe { cl_consp(cursor) } == ECL_NIL {
            return Err(format!("{name} must be a proper list"));
        }
        if values.len() == maximum {
            return Err(format!("{name} has more than {maximum} items"));
        }
        if seen.contains(&cursor) {
            return Err(format!("{name} cannot be circular"));
        }
        seen.push(cursor);
        values.push(unsafe { cl_car(cursor) });
        cursor = unsafe { cl_cdr(cursor) };
    }
    Ok(values)
}

fn decode_tone_sequence(object: ClObject) -> Result<Vec<audio::Tone>, String> {
    let notes = decode_object_list(object, "tone sequence", 3)?;
    if !(1..=3).contains(&notes.len()) {
        return Err("tone sequence needs one through three notes".to_owned());
    }
    notes
        .into_iter()
        .map(|note| {
            let pair = decode_object_list(note, "tone", 2)?;
            let [frequency, duration_ms] = pair.as_slice() else {
                return Err("each tone needs frequency and duration".to_owned());
            };
            Ok(audio::Tone {
                frequency: decode_i32(*frequency, "tone frequency")?,
                duration_ms: decode_i32(*duration_ms, "tone duration")?,
            })
        })
        .collect()
}

fn decode_os_string(object: ClObject, name: &str) -> Result<OsString, String> {
    if unsafe { cl_stringp(object) } == ECL_NIL {
        return Err(format!("{name} must be a string"));
    }
    let bytes = decode_base_string(object, name)?;
    if bytes.contains(&0) {
        return Err(format!("{name} cannot contain NUL"));
    }
    Ok(OsString::from_vec(bytes))
}

fn decode_os_string_list(object: ClObject, name: &str) -> Result<Vec<OsString>, String> {
    decode_object_list(object, name, usize::MAX)?
        .into_iter()
        .enumerate()
        .map(|(index, value)| decode_os_string(value, &format!("{name} item {index}")))
        .collect()
}

fn decode_environment(object: ClObject) -> Result<Vec<(OsString, OsString)>, String> {
    decode_object_list(object, "child environment", usize::MAX)?
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            if unsafe { cl_consp(entry) } == ECL_NIL {
                return Err(format!("child environment item {index} must be a pair"));
            }
            let key = decode_os_string(
                unsafe { cl_car(entry) },
                &format!("child environment key {index}"),
            )?;
            let value = decode_os_string(
                unsafe { cl_cdr(entry) },
                &format!("child environment value {index}"),
            )?;
            Ok((key, value))
        })
        .collect()
}

fn decode_fixnum(object: ClObject) -> Option<ClFixnum> {
    let tagged = object as usize;
    (tagged & 3 == FIXNUM_TAG).then_some((tagged as isize) >> 2)
}

fn decode_boolean_fixnum(object: ClObject, name: &str) -> Result<bool, String> {
    match decode_fixnum(object) {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(format!("{name} must be zero or one")),
    }
}

fn decode_i32(object: ClObject, name: &str) -> Result<c_int, String> {
    let value = decode_fixnum(object).ok_or_else(|| format!("{name} must be an integer"))?;
    c_int::try_from(value).map_err(|_| format!("{name} is out of range"))
}

fn decode_i64_hex(object: ClObject, name: &str) -> Result<i64, String> {
    let bytes = decode_base_string(object, name)?;
    if bytes.len() != 16 || !bytes.iter().all(u8::is_ascii_hexdigit) {
        return Err(format!("{name} must contain sixteen hexadecimal digits"));
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| format!("{name} is not ASCII"))?;
    let value = u64::from_str_radix(text, 16).map_err(|_| format!("{name} is out of range"))?;
    i64::try_from(value).map_err(|_| format!("{name} is out of range"))
}

fn decode_u32(object: ClObject, name: &str) -> Result<u32, String> {
    let value = decode_fixnum(object).ok_or_else(|| format!("{name} must be an integer"))?;
    u32::try_from(value).map_err(|_| format!("{name} is out of range"))
}

fn decode_color(object: ClObject, name: &str) -> Result<u32, String> {
    let color = decode_u32(object, name)?;
    (color <= 0x00ff_ffff)
        .then_some(color)
        .ok_or_else(|| format!("{name} is out of range"))
}

fn decode_exit_code(object: ClObject) -> Result<u8, String> {
    let value = decode_fixnum(object)
        .ok_or_else(|| "RETRODECK:MAIN must return an integer exit status".to_owned())?;
    u8::try_from(value).map_err(|_| "RETRODECK:MAIN returned an invalid exit status".to_owned())
}

fn parse_uploader_password_config(path: &OsStr) -> Result<(u32, Vec<u8>, Vec<u8>), String> {
    let path = Path::new(path);
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect uploader password configuration: {error}"))?;
    if !metadata.file_type().is_file()
        || metadata.mode() & 0o077 != 0
        || unsafe { libc::geteuid() } == 0 && metadata.uid() != 0
    {
        return Err("uploader password configuration must be private and root-owned".to_owned());
    }
    let bytes = regular_file::read_regular(path, 1, 1024, "uploader password configuration")?
        .ok_or_else(|| "uploader password configuration is missing".to_owned())?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "uploader password configuration is not UTF-8".to_owned())?;
    let lines = text.split('\n').collect::<Vec<_>>();
    if lines.len() != 5 || lines[0] != "version=1" || !lines[4].is_empty() {
        return Err("uploader password configuration has an invalid schema".to_owned());
    }
    let (mut iterations, mut salt, mut digest) = (None, None, None);
    for line in &lines[1..4] {
        let (name, value) = line
            .split_once('=')
            .ok_or_else(|| "uploader password configuration has a malformed field".to_owned())?;
        let destination = match name {
            "iterations" => &mut iterations,
            "salt" => &mut salt,
            "digest" => &mut digest,
            _ => return Err("uploader password configuration has an unknown field".to_owned()),
        };
        if destination.replace(value).is_some() {
            return Err("uploader password configuration repeats a field".to_owned());
        }
    }
    let iterations = iterations
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| (100_000..=1_000_000).contains(value))
        .ok_or_else(|| "uploader password configuration has invalid iterations".to_owned())?;
    let salt = STANDARD_NO_PAD
        .decode(salt.unwrap_or_default())
        .map_err(|_| "uploader password configuration has an invalid salt".to_owned())?;
    let digest = STANDARD_NO_PAD
        .decode(digest.unwrap_or_default())
        .map_err(|_| "uploader password configuration has an invalid digest".to_owned())?;
    if salt.len() != 16 || digest.len() != 32 {
        return Err("uploader password configuration has invalid byte lengths".to_owned());
    }
    Ok((iterations, salt, digest))
}

fn verify_uploader_password(path: &OsStr) -> Result<u8, String> {
    let (iterations, salt, digest) = parse_uploader_password_config(path)?;
    let mut password = Vec::new();
    std::io::stdin()
        .take(129)
        .read_to_end(&mut password)
        .map_err(|error| format!("cannot read uploader password: {error}"))?;
    if password.len() > 128 {
        return Ok(1);
    }
    let candidate = pbkdf2_hmac_array::<Sha256, 32>(&password, &salt, iterations);
    Ok(u8::from(!bool::from(
        candidate.as_slice().ct_eq(digest.as_slice()),
    )))
}

fn sibling_startup(program: Option<&OsStr>) -> PathBuf {
    // Prefer the lisp tree installed next to the resolved executable so a
    // staged payload validates against its own sources before activation.
    let resolved = program
        .map(Path::new)
        .and_then(|program| std::fs::canonicalize(program).ok());
    if let Some(startup) = resolved
        .as_deref()
        .and_then(Path::parent)
        .map(|parent| parent.join("lisp/startup.lisp"))
        .filter(|startup| startup.is_file())
    {
        return startup;
    }
    PathBuf::from(DEFAULT_STARTUP)
}

fn explicit_startup() -> Result<Option<PathBuf>, String> {
    let mut arguments = env::args_os();
    let program = arguments.next();
    // App symlinks (deck-menu, chiptune-deck, ...) keep their arguments for
    // the Lisp router; only direct invocation selects a startup file.
    let dedicated = program
        .as_deref()
        .map(Path::new)
        .and_then(Path::file_name)
        .is_some_and(|name| name != "retrodeck-native");
    if dedicated {
        return Ok(None);
    }
    match (arguments.next(), arguments.next()) {
        (None, None) => Ok(None),
        (Some(path), None) => Ok(Some(path.into())),
        _ => Err("usage: retrodeck-native [STARTUP.LISP]".to_owned()),
    }
}

fn startup_path() -> Result<PathBuf, String> {
    Ok(match explicit_startup()? {
        Some(path) => path,
        None => sibling_startup(env::args_os().next().as_deref()),
    })
}

/// Loads the Lisp tree: an explicit startup file or RETRO_DECK_LISP_SOURCE
/// forces the source path; otherwise the compiled-in image initializes and a
/// sibling local.lisp, when present, overlays it for hot patches.
#[cfg(feature = "lisp-image")]
fn load_lisp(ecl: &Ecl) -> Result<(), String> {
    if let Some(path) = explicit_startup()? {
        return ecl.load(&path);
    }
    if env::var_os("RETRO_DECK_LISP_SOURCE").is_some() {
        return ecl.load(&startup_path()?);
    }
    ecl.initialize_image();
    let local = sibling_startup(env::args_os().next().as_deref()).with_file_name("local.lisp");
    if local.is_file() {
        return ecl.load(&local);
    }
    Ok(())
}

#[cfg(not(feature = "lisp-image"))]
fn load_lisp(ecl: &Ecl) -> Result<(), String> {
    ecl.load(&startup_path()?)
}

fn run() -> Result<u8, String> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let flag = arguments.next();
    if let Some(flag) = flag.as_deref().and_then(OsStr::to_str)
        && matches!(
            flag,
            "--verify-uploader-password" | "--check-uploader-password-config"
        )
    {
        let usage = || format!("usage: retrodeck-native {flag} PASSWORD.CONF");
        let path = arguments.next().ok_or_else(usage)?;
        if arguments.next().is_some() {
            return Err(usage());
        }
        if flag == "--check-uploader-password-config" {
            parse_uploader_password_config(&path)?;
            return Ok(0);
        }
        return verify_uploader_password(&path);
    }
    let ecl = Ecl::boot()?;
    process::install_signal_handlers()?;
    ecl.register_primitives()?;
    load_lisp(&ecl)?;
    // RETRO_DECK_REPL drops into an interactive top level over the loaded
    // tree (the compiled image in production builds) instead of MAIN.
    if env::var_os("RETRO_DECK_REPL").is_some() {
        return ecl.repl();
    }
    ecl.run()
}

fn main() -> ExitCode {
    match run() {
        Ok(status) => ExitCode::from(status),
        Err(error) => {
            eprintln!("retrodeck: {error}");
            ExitCode::FAILURE
        }
    }
}
