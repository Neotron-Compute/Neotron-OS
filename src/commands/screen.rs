//! Screen-related commands for Neotron OS

use neotron_common_bios::video::{Attr, TextBackgroundColour, TextForegroundColour};

use crate::{print, println, Ctx, API, VGA_CONSOLE};

pub static CLEAR_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: clear,
        parameters: &[],
    },
    command: "screen_clear",
    help: Some("Clear the screen"),
};

pub static FILL_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: fill,
        parameters: &[],
    },
    command: "screen_fill",
    help: Some("Fill the screen with characters"),
};

pub static BENCH_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: bench,
        parameters: &[],
    },
    command: "screen_bench",
    help: Some("Time how long to put 1,000,000 characters on the screen, with scrolling."),
};

pub static MANDEL_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: mandel,
        parameters: &[],
    },
    command: "screen_mandel",
    help: Some("Calculate the Mandelbrot set"),
};

/// Called when the "clear" command is executed.
fn clear(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    if let Some(ref mut console) = unsafe { &mut VGA_CONSOLE } {
        console.clear();
    }
}

/// Called when the "fill" command is executed.
fn fill(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    if let Some(ref mut console) = unsafe { &mut VGA_CONSOLE } {
        console.clear();
        let api = API.get();
        let mode = (api.video_get_mode)();
        let (Some(width), Some(height)) = (mode.text_width(), mode.text_height()) else {
            println!("Unable to get console size");
            return;
        };
        // A range of printable ASCII compatible characters
        let mut char_cycle = (b' '..=b'~').cycle();
        let mut remaining = height * width;

        // Scroll two screen fulls
        'outer: for bg in (0..=7).cycle() {
            let bg_colour = TextBackgroundColour::new(bg).unwrap();
            for fg in 1..=15 {
                if fg == bg {
                    continue;
                }
                let fg_colour = TextForegroundColour::new(fg).unwrap();
                remaining -= 1;
                if remaining == 0 {
                    break 'outer;
                }
                let attr = Attr::new(fg_colour, bg_colour, false);
                let glyph = char_cycle.next().unwrap();
                console.set_attr(attr);
                console.write_bstr(&[glyph]);
            }
        }
        let attr = Attr::new(
            TextForegroundColour::WHITE,
            TextBackgroundColour::BLACK,
            false,
        );
        console.set_attr(attr);
    }
}

/// Called when the "bench" command is executed.
fn bench(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    const NUM_CHARS: u64 = 1_000_000;
    if let Some(ref mut console) = unsafe { &mut VGA_CONSOLE } {
        let api = API.get();
        let start = (api.time_ticks_get)();
        console.clear();
        let glyphs = &[b'x'];
        for _idx in 0..NUM_CHARS {
            console.write_bstr(glyphs);
        }
        let end = (api.time_ticks_get)();
        let delta = end.0 - start.0;
        let chars_per_second = (NUM_CHARS * (api.time_ticks_per_second)().0) / delta;
        println!(
            "{} chars in {} ticks, or {} chars per second",
            NUM_CHARS, delta, chars_per_second
        );
    }
}

/// Called when the "mandel" command is executed.
fn mandel(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    fn mandelbrot(cx: f64, cy: f64, max_loops: u32) -> u32 {
        let mut x = cx;
        let mut y = cy;
        for i in 1..max_loops {
            let x_squared = x * x;
            let y_squared = y * y;
            if x_squared + y_squared > 4.0 {
                return i;
            }
            let x1 = x_squared - y_squared + cx;
            let y1 = (2.0 * x * y) + cy;
            x = x1;
            y = y1;
        }
        0
    }

    let api = API.get();
    let mode = (api.video_get_mode)();
    let (Some(width), Some(height)) = (mode.text_width(), mode.text_height()) else {
        println!("Unable to get screen size");
        return;
    };

    let glyphs = b" .,'~!^:;[/<&?oxOX#  ";
    for y_pos in 0..height - 2 {
        let y = (f64::from(y_pos) * 4.0 / f64::from(height)) - 2.0;
        for x_pos in 0..width {
            let x = (f64::from(x_pos) * 4.0 / f64::from(width)) - 2.0;
            let result = mandelbrot(x, y, 20);
            print!("{}", glyphs[result as usize] as char);
        }
    }
}
