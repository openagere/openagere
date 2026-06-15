# Headers, primary, and secondary text

- **Headers:** Use `bold`. For markdown with various header levels, leave in the `#` signs.
- **Primary text:** Default.
- **Secondary text:** Use `dim`.

# Foreground colors

- **Default:** Most of the time, just use the default foreground color. `reset` can help get it back.
- **User input tips, selection, and status indicators:** Use ANSI `cyan`.
- **Success and additions:** Use ANSI `green`.
- **Errors, failures and deletions:** Use ANSI `red`.
- **Agere:** Use ANSI `magenta`.

# Avoid

- Avoid custom colors because there's no guarantee that they'll contrast well or look good in various terminal color themes. (`shimmer.rs` is an exception that works well because we take the default colors and just adjust their levels.)
- Avoid ANSI `black` & `white` as foreground colors because the default terminal theme color will do a better job. (Use `reset` if you need to in order to get those.) The exception is if you need contrast rendering over a manually colored background.
- Avoid ANSI `blue` and `yellow` because for now the style guide doesn't use them. Prefer a foreground color mentioned above.

(There are some rules to try to catch this in `clippy.toml`.)

# Neon Colors

Neon RGB colors are used for framed UI regions to visually differentiate from the default terminal style. These colors bypass ANSI palette quantization and use direct RGB values for maximum contrast.

- **User message frames:** Cyan Neon `Color::Rgb(0x00, 0xFF, 0xFF)`
- **AI reply frames:** Magenta Neon `Color::Rgb(0xFF, 0x00, 0xFF)`
- **Bottom bar / footer frames:** Gray `Color::Rgb(0x60, 0x60, 0x60)`
- **Popup / modal frames:** Purple Neon `Color::Rgb(0xBF, 0x00, 0xFF)`
- **Separator lines:** Dim Gray `Color::Rgb(0x40, 0x40, 0x40)`

All neon frames use `BorderType::Rounded` (Unicode rounded corners: ╭ ╮ ╰ ╯).
