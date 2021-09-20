# Contributing

🎉 Thanks for contributing to electron-collider! 🎉

This project adheres to the Contributor Covenant code of conduct. By participating, you are expected to uphold this code. Please report unacceptable behavior to coc@electronjs.org.

The following is a set of guidelines for contributing to Electron-Collider. This Contributing doc is very much a work in progress, to be fleshed out as contributors work. Please feel
free to add your own comments and types, and edit freely.

### Installing Rust

If you're an Electron developer, you may be more familiar with JavaScript and C++. This section is meant to be a short intro
for what you'll need to develop in Rust, for someone new to the ecosystem. If you're already a Rust developer, feel free to skip
this section.

Use `rustup` to set up Rust and the Cargo dependency.

If using VSCode, use the `rust-analyzer` extension for Rust support, rather than The Rust Programming language extension.


### Setup

Fork the project on GitHub and clone your fork locally.

```
$ git clone git@github.com:username/electron-collider.git
$ cd electron-collider
$ git remote add upstream https://github.com/zkat/electron-collider.git
$ git fetch upstream
```

Cargo automatically installs and builds dependencies.

Run `cargo run -- -h` to ensure that Cargo has been installed. This will display the help menu, showing you what flags and subcommands are available.

### Commands

Commands are the {WIP}. commands: 

All commands follow the naming convention `collider-cmd-{cmdname}`.

### Adding a new subcommand

If you'd like to add a new subcommand to Electron Collider, you'll need to update the code in the following areas:

1. Add it to the enum
2. Add it to the ColliderCommand impl on line 128
3. Add it to the ConfigLayer implementation

### Debugging

`command-start` hits the GitHub API if you don't already have Electron installed. If you see a rate limit error, that may be why.

If you encounter a bad build and need to reset your dev build, blow away the `target` directory or `release` directory, depending on the mode that you're building in.