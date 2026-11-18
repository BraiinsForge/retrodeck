//! Render the settings reference fixture without starting the dashboard runtime.
use retrodeck_native::{
    canvas,
    dashboard::{policy, settings, ui},
    network::NetworkStatus,
};
use std::{error::Error, fs::File};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() != 1 {
        return Err("usage: settings-frame OUTPUT.png".into());
    }
    ui::install_palette(
        policy::DEFAULT_PALETTE_VALUES
            .iter()
            .map(|(name, color)| ((*name).into(), *color))
            .collect(),
    );
    let network = NetworkStatus {
        ssid: "net1".into(),
        wlan_ipv4: "10.249.110.248".into(),
        wireguard_ipv4: "10.0.0.10".into(),
        selector: "CONNECTED".into(),
    };
    settings::render_frame(&settings::State::default(), &network);
    let mut encoder =
        png::Encoder::new(File::create(&arguments[0])?, canvas::WIDTH, canvas::HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    canvas::with_pixels(|pixels| writer.write_image_data(pixels))?;
    Ok(())
}
