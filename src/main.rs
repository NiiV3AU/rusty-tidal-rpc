#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod artwork;
mod discord;
mod media;

use std::{
    env,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use artwork::ArtworkResolver;
use discord::DiscordPresence;
use media::TidalMedia;
use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem},
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostQuitMessage, TranslateMessage,
};

const DEFAULT_DISCORD_APP_ID: &str = "584458858731405315";
const POLL_INTERVAL: Duration = Duration::from_secs(2);

fn main() -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let enabled = Arc::new(AtomicBool::new(true));

    let worker_running = Arc::clone(&running);
    let worker_enabled = Arc::clone(&enabled);
    let worker = thread::spawn(move || {
        if let Err(error) = run_rpc(worker_running, worker_enabled) {
            eprintln!("TIDAL RPC stopped: {error:#}");
        }
    });

    let enabled_item = CheckMenuItem::with_id(
        "presence-enabled",
        "Discord presence enabled",
        true,
        true,
        None,
    );
    let exit_item = MenuItem::with_id("exit", "Exit", true, None);
    let menu = Menu::with_items(&[&enabled_item, &exit_item])?;
    let _tray = TrayIconBuilder::new()
        .with_tooltip("TIDAL Discord RPC")
        .with_icon(tidal_icon()?)
        .with_menu(Box::new(menu))
        .build()?;

    let menu_running = Arc::clone(&running);
    let menu_enabled = Arc::clone(&enabled);
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id.0.as_str() {
        "presence-enabled" => {
            menu_enabled.fetch_xor(true, Ordering::SeqCst);
        }
        "exit" => {
            menu_running.store(false, Ordering::SeqCst);
            unsafe { PostQuitMessage(0) };
        }
        _ => {}
    }));

    run_message_loop();
    running.store(false, Ordering::SeqCst);
    let _ = worker.join();
    Ok(())
}

fn run_rpc(running: Arc<AtomicBool>, enabled: Arc<AtomicBool>) -> Result<()> {
    unsafe {
        windows::Win32::System::WinRT::RoInitialize(
            windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
        )
    }
    .context("could not initialize the Windows Runtime")?;
    let _winrt = WinRtGuard;

    let app_id =
        env::var("TIDAL_RPC_DISCORD_APP_ID").unwrap_or_else(|_| DEFAULT_DISCORD_APP_ID.to_owned());
    let media = TidalMedia::connect().context("could not access Windows media sessions")?;
    let mut artwork = ArtworkResolver::new()?;
    let mut discord = DiscordPresence::new(app_id);

    while running.load(Ordering::Relaxed) {
        if !enabled.load(Ordering::Relaxed) {
            discord.clear();
            thread::sleep(POLL_INTERVAL);
            continue;
        }

        match media.current() {
            Ok(Some(mut track)) => {
                if track.artwork_url.is_none() {
                    track.artwork_url = artwork.resolve(&track);
                }
                discord.publish(&track);
            }
            Ok(None) => discord.clear(),
            Err(error) => {
                eprintln!("TIDAL media read failed: {error:#}");
                discord.clear();
            }
        }
        thread::sleep(POLL_INTERVAL);
    }

    discord.clear();
    Ok(())
}

fn run_message_loop() {
    let mut message = MSG::default();
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn tidal_icon() -> Result<Icon> {
    const SIZE: u32 = 32;
    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];
    for center_y in [10_i32, 20] {
        for center_x in [10_i32, 20] {
            for y in 0..SIZE as i32 {
                for x in 0..SIZE as i32 {
                    if (x - center_x).abs() + (y - center_y).abs() <= 5 {
                        let offset = ((y as u32 * SIZE + x as u32) * 4) as usize;
                        rgba[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
                    }
                }
            }
        }
    }
    Ok(Icon::from_rgba(rgba, SIZE, SIZE)?)
}

struct WinRtGuard;

impl Drop for WinRtGuard {
    fn drop(&mut self) {
        unsafe { windows::Win32::System::WinRT::RoUninitialize() };
    }
}
