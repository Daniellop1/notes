VIM NOTES TUI
=============

A high-performance, minimalist, and deeply customizable Terminal UI note-taking application 
written entirely in Rust. Powered by 'crossterm' and 'ratatui', this application brings the 
speed and ergonomics of Vim to your daily note management.


*** KEY FEATURES ***
--------------------
* Modal Editing Architecture: Native Vim-like keybindings with dedicated Normal, Insert, Visual, and Command modes.
* Dual-Pane Interface: A fully responsive sidebar for file exploration and a central editor for rapid writing.
* Zen Mode: Press 'z' to temporarily banish the file explorer, expanding the editor to 100% of the screen for distraction-free writing.
* Global Omni-Search: A blistering-fast search protocol ('/') that filters files not just by their filename, but by parsing the exact content inside your Markdown files globally. 
* Real-Time Markdown Engine: The application dynamically highlights your syntax (Headings, Quotes, Lists) while you type. Matches from your search queries are illuminated in bright yellow traversing the text.
* Productivity Overlays: 
    > Hit 'Space' over any checkbox ('- [ ]') in Normal Mode to instantly mark it as complete ('- [x]') and hard-save to disk.
    > Hit 'Ctrl + I' to deploy the Document Statistics Overlay, keeping track of Byte Size, Word count, Character metrics, and estimated reading times.
* Visual Selection Mode: Enter visual mode ('v') to anchor a cursor point, select paragraphs with your directional keys, Yank ('y') to the internal clipboard, Cut ('d'), and Paste ('p') wherever you like.
* Dynamic Theming Engine: Write ':theme hacker', ':theme ocean', ':theme light', or ':theme default' in the Command Mode. Watch the application's borders, UI bounds, and text parameters dynamically morph according to your visual preference during runtime.
* Invisible Persistence: Notes are automatically autosaved to an organized, isolated path inside '~/.local/share/mynotes/'. Every switch, quit, or application exit safely preserves your progress seamlessly.


*** INSTALLATION ***
--------------------
As the repository is standard Rust, ensuring you have the Rust 'cargo' toolchain 
installed is all that's required:

    git clone <repository_url>
    cd notes
    cargo run
    (Or compile for release: cargo build --release)


*** INTERNAL FILE STORAGE ***
-----------------------------
All notes are automatically tracked and saved utilizing the cross-platform 'home' crate.
Your notes can always be manually found, backed up, or manipulated externally here:
~/.local/share/mynotes/


*** KEYBINDINGS MAP ***
-----------------------
Press '?' inside the application to invoke the Help Overlay at any time.

[Global & File Management]
    j / Down : Traverse file list downward
    k / Up   : Traverse file list upward
    Enter    : Load highlighted file into Editor
    n        : Create a new .md Note
    r        : Rename the currently active Note
    d        : Hard-delete the active Note
    /        : Trigger Global search/filter query
    z        : Toggle Zen (Focus) Mode 

[Editor & Navigation]
    i        : Enter Insert Mode
    v        : Enter Visual Mode (Selection Engine)
    Esc      : Drop back into Normal Mode (Aborts visual/insert operations)
    Space    : Toggle Checkbox sequence ('- [ ]' to '- [x]')
    Ctrl + I : Open exact Document Statistics module

[Visual Selection Context]
    y           : Yank (Copy) highlighted area to App clipboard
    d           : Cut highlighted area into clipboard, removing from Buffer.
    p           : Paste clipboard into the buffer cursor location.
    Arrow Keys  : Adjust Visual Anchor boundaries.

[Command Integrations]
    :w (or C-s) : Force manual save.
    :q (or C-c) : Auto-save & Terminate application.
    :wq         : Save & Terminate application.
    :theme name : Execute theme morph (hacker, ocean, light, default).

--------------------------------------------------------------------------------
A minimalist, terminal-first environment constructed with explicit focus on developer ergonomics.
