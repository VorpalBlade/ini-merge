# Library to merge INI files subject to configuration

[ [crates.io] ] [ [lib.rs] ] [ [docs.rs] ]

This library forms the backend to [chezmoi_modify_manager]. You probably
want that tool instead.

This library provides processing of INI files. In particular:

* Merging of a source INI file with a target INI file.
  The merging is asymmetric: The values of the source are preferred unless
  specific rules have been provided for those sections and/or keys.
  Formatting is preserved. See [merge_ini].

The use case of this is configuration management for user settings file where
the program writes a mix of settings and state to the same file. This gets
messy if we want to track the settings part in git using a tool like [chezmoi].

A typical example of this is KDE settings files. These contain (apart from
settings) state like recently opened files and positions of windows and dialog
boxes. Other programs (such as PrusaSlicer) also do the same thing.

This library can be used as a backend to implement a tool to smartly merge
such INI files. Such a tool is already available: [chezmoi_modify_manager].

[chezmoi_modify_manager]: https://github.com/VorpalBlade/chezmoi_modify_manager
[chezmoi]: https://www.chezmoi.io/
[crates.io]: https://crates.io/crates/ini-merge
[docs.rs]: https://docs.rs/ini-merge
[lib.rs]: https://lib.rs/crates/ini-merge
