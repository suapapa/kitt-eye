fn main() {
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-changed=.env.local");

    if let Ok(content) = std::fs::read_to_string(".env") {
        println!("cargo:warning=Loading configuration from .env file");
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim().trim_matches('"').trim_matches('\'');
                // SAFETY: build script is single-threaded; set_var is used only here.
                unsafe {
                    std::env::set_var(key, val);
                }
            }
        }
    }

    linker_be_nice();
    println!("cargo:rustc-link-arg=-Tlinkall.x");

    // Wi-Fi
    emit_env("SSID", None, None);
    emit_env("PASS", Some("PASSWORD"), None);

    // Motion theme (classic = red-only state→motion map)
    emit_env("MOTION_THEME", None, Some("classic"));

    // MQTT
    emit_env("MQTT_BROKER", None, None);
    emit_env("MQTT_PORT", None, Some("1883"));
    emit_env("MQTT_USER", None, Some(""));
    emit_env("MQTT_PASS", Some("MQTT_PASSWORD"), Some(""));
    emit_env("MQTT_TOPIC_PREFIX", None, Some("kitt-eye"));
    emit_env("MQTT_CLIENT_ID", None, Some("kitt-eye-mcu"));
}

fn emit_env(primary: &str, alias: Option<&str>, default: Option<&str>) {
    println!("cargo:rerun-if-env-changed={primary}");
    if let Some(alias) = alias {
        println!("cargo:rerun-if-env-changed={alias}");
    }

    let value = std::env::var(primary)
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| alias.and_then(|a| std::env::var(a).ok().filter(|v| !v.is_empty())))
        .or_else(|| default.map(str::to_string))
        .unwrap_or_default();

    if value.is_empty() && default.is_none() {
        eprintln!("cargo:warning={primary} is unset; firmware will use an empty placeholder");
    }
    println!("cargo:rustc-env={primary}={value}");
}

fn linker_be_nice() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let kind = &args[1];
        let what = &args[2];

        match kind.as_str() {
            "undefined-symbol" => match what.as_str() {
                what if what.starts_with("_defmt_") => {
                    eprintln!();
                    eprintln!(
                        "💡 `defmt` not found - make sure `defmt.x` is added as a linker script and you have included `use defmt_rtt as _;`"
                    );
                    eprintln!();
                }
                "_stack_start" => {
                    eprintln!();
                    eprintln!("💡 Is the linker script `linkall.x` missing?");
                    eprintln!();
                }
                what if what.starts_with("esp_rtos_") => {
                    eprintln!();
                    eprintln!(
                        "💡 `esp-radio` has no scheduler enabled. Make sure you have initialized `esp-rtos` or provided an external scheduler."
                    );
                    eprintln!();
                }
                "embedded_test_linker_file_not_added_to_rustflags" => {
                    eprintln!();
                    eprintln!(
                        "💡 `embedded-test` not found - make sure `embedded-test.x` is added as a linker script for tests"
                    );
                    eprintln!();
                }
                "free"
                | "malloc"
                | "calloc"
                | "get_free_internal_heap_size"
                | "malloc_internal"
                | "realloc_internal"
                | "calloc_internal"
                | "free_internal" => {
                    eprintln!();
                    eprintln!(
                        "💡 Did you forget the `esp-alloc` dependency or didn't enable the `compat` feature on it?"
                    );
                    eprintln!();
                }
                _ => (),
            },
            _ => {
                std::process::exit(1);
            }
        }

        std::process::exit(0);
    }

    println!(
        "cargo:rustc-link-arg=--error-handling-script={}",
        std::env::current_exe().unwrap().display()
    );
}
