//! mk-probe: read-only diagnostics for Apple keyboards. It never writes output or feature
//! reports to the device.

#[cfg(not(windows))]
fn main() {
    eprintln!("mk-probe only runs on Windows");
}

#[cfg(windows)]
mod rawinput;

#[cfg(windows)]
mod probe {
    use std::io;
    use std::time::Instant;

    use mk_hid::{battery, hex, Collection, Device, ReportKind};

    pub const USAGE: &str = "\
usage: mk-probe <command>

  list [--all]        HID collections of Apple keyboards (--all: every HID collection)
  caps [TAG]          button/value capabilities (TAG filters by e.g. MI_01&Col01)
  watch TAG           print input reports of one collection as they arrive (Ctrl+C stops)
  battery             current battery state of every Apple keyboard
  feature TAG ID      read one feature report, e.g. `feature MI_01&Col02 0x09` (never writes)
  input-monitor       show what Windows receives (Raw Input keyboard + consumer control)";

    fn select(tag: Option<&str>) -> io::Result<Vec<Collection>> {
        let all = mk_hid::apple_collections()?;
        Ok(match tag {
            None => all,
            Some(t) => {
                let t = t.to_ascii_uppercase();
                all.into_iter()
                    .filter(|c| c.tag().to_ascii_uppercase().contains(&t))
                    .collect()
            }
        })
    }

    fn select_one(tag: &str) -> io::Result<Collection> {
        let mut found = select(Some(tag))?;
        if found.len() > 1 {
            eprintln!(
                "note: {} collections match {tag}; using the first",
                found.len()
            );
        }
        if found.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no Apple collection matches {tag}"),
            ));
        }
        Ok(found.swap_remove(0))
    }

    fn parse_id(s: &str) -> Option<u8> {
        match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            Some(h) => u8::from_str_radix(h, 16).ok(),
            None => s.parse().ok(),
        }
    }

    pub fn list(all: bool) -> io::Result<()> {
        let cols = if all {
            mk_hid::all_collections()?
        } else {
            mk_hid::apple_collections()?
        };
        if cols.is_empty() {
            println!("no collections found");
        }
        for c in &cols {
            let model = c.model().map(|m| m.name).unwrap_or("?");
            println!("{c}  {model}");
            println!("    {}", c.path);
        }
        Ok(())
    }

    pub fn caps(tag: Option<&str>) -> io::Result<()> {
        for c in select(tag)? {
            println!("{c}");
            let dev = match Device::open(&c, false) {
                Ok(d) => d,
                Err(e) => {
                    println!("  cannot open: {e}");
                    continue;
                }
            };
            for kind in ReportKind::ALL {
                let mut pages: Vec<u16> =
                    dev.button_caps(kind).iter().map(|b| b.usage_page).collect();
                pages.sort_unstable();
                pages.dedup();
                for p in pages {
                    println!(
                        "  {kind:?} page {p:04X}: max {} usages per report",
                        dev.max_usages(kind, p)
                    );
                }
                for b in dev.button_caps(kind) {
                    println!(
                        "  {kind:?} button id={:02X} page={:04X} usage={:04X}..{:04X} {} count={} lc={} idx={}..{}",
                        b.report_id,
                        b.usage_page,
                        b.usage_min,
                        b.usage_max,
                        if b.is_range { "range" } else { "single" },
                        b.report_count,
                        b.link_collection,
                        b.data_index_min,
                        b.data_index_max,
                    );
                }
                for v in dev.value_caps(kind) {
                    println!(
                        "  {kind:?} value  id={:02X} page={:04X} usage={:04X}..{:04X} bits={} count={} logical={}..{} lc={}",
                        v.report_id,
                        v.usage_page,
                        v.usage_min,
                        v.usage_max,
                        v.bit_size,
                        v.report_count,
                        v.logical_min,
                        v.logical_max,
                        v.link_collection,
                    );
                }
            }
        }
        Ok(())
    }

    pub fn watch(tag: &str) -> io::Result<()> {
        let c = select_one(tag)?;
        let dev = Device::open(&c, true).map_err(|e| {
            io::Error::new(e.kind(), format!("{e} (the keyboard collection is opened exclusively by Windows; watch Col02/battery instead)"))
        })?;
        let mut pages: Vec<u16> = dev
            .button_caps(ReportKind::Input)
            .iter()
            .map(|b| b.usage_page)
            .collect();
        pages.sort_unstable();
        pages.dedup();
        println!("watching {c}");
        let start = Instant::now();
        loop {
            let r = dev.read_report()?;
            let mut line = format!("{:>9.3}s  {}", start.elapsed().as_secs_f64(), hex(&r));
            for &p in &pages {
                let u = dev.usages(ReportKind::Input, p, &r);
                if !u.is_empty() {
                    let list: Vec<String> = u.iter().map(|x| format!("{x:02X}")).collect();
                    line += &format!("  [{p:04X}: {}]", list.join(","));
                }
            }
            if c.is_battery() {
                if let Some(b) = battery::decode(&dev, &r) {
                    line += &format!("  {b:?}");
                }
            }
            println!("{line}");
        }
    }

    pub fn battery() -> io::Result<()> {
        let cols = battery::collections()?;
        if cols.is_empty() {
            println!("no Apple battery collection found");
        }
        for c in cols {
            match battery::read(&c) {
                Ok(b) => println!(
                    "{:04X}:{:04X} {}  {}%{}{}",
                    c.vid,
                    c.pid,
                    if c.is_bluetooth() { "BT" } else { "USB" },
                    b.percent,
                    if b.charging { " charging" } else { "" },
                    if b.fully_charged { " full" } else { "" },
                ),
                Err(e) => println!("{:04X}:{:04X} {}  error: {e}", c.vid, c.pid, c.tag()),
            }
        }
        Ok(())
    }

    pub fn feature(tag: &str, id: &str) -> io::Result<()> {
        let id = parse_id(id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad report ID"))?;
        let c = select_one(tag)?;
        let dev = Device::open(&c, true).or_else(|_| Device::open(&c, false))?;
        let r = dev.get_feature(id)?;
        println!("{c}\nfeature {id:02X}: {}", hex(&r));
        Ok(())
    }
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match a.as_slice() {
        ["list"] => probe::list(false),
        ["list", "--all"] => probe::list(true),
        ["caps"] => probe::caps(None),
        ["caps", tag] => probe::caps(Some(tag)),
        ["watch", tag] => probe::watch(tag),
        ["battery"] => probe::battery(),
        ["feature", tag, id] => probe::feature(tag, id),
        ["input-monitor"] => rawinput::run(),
        _ => {
            eprintln!("{}", probe::USAGE);
            return std::process::ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
