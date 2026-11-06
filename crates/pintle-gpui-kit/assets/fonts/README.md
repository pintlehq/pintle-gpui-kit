# Bundled fonts

JetBrains Mono is distributed under the SIL Open Font License 1.1 in `OFL.txt`.
The kit's opt-in `assets` feature registers Regular, Medium, SemiBold and Italic
once during facade initialization. Applications can override the mono font in
`ThemeOptions`; without bundled assets GPUI uses platform font fallback.
