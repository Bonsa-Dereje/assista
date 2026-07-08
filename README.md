# Assista

A toolbar built for 2-in-1 laptops, designed to help users get the most out of tablet mode and smooth out the small inconveniences that come with it.

<p>
  <img src="https://img.shields.io/badge/Tauri-24C8DB?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri" />
  <img src="https://img.shields.io/badge/Svelte-FF3E00?style=for-the-badge&logo=svelte&logoColor=white" alt="Svelte" />
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/JavaScript-F7DF1E?style=for-the-badge&logo=javascript&logoColor=black" alt="JavaScript" />
  <img src="https://img.shields.io/badge/Vite-646CFF?style=for-the-badge&logo=vite&logoColor=white" alt="Vite" />
  <img src="https://img.shields.io/badge/HTML5-E34F26?style=for-the-badge&logo=html5&logoColor=white" alt="HTML5" />
</p>

## About

Assista is a lightweight desktop toolbar aimed at 2-in-1 laptop users, whose devices switch between laptop and tablet modes. It addresses the friction that often appears in tablet mode — quick access to common actions, better touch ergonomics, and small quality-of-life fixes that the operating system doesn't handle out of the box.

The app is built with [Tauri](https://tauri.app), pairing a Rust backend with a [SvelteKit](https://kit.svelte.dev) frontend for a fast, lightweight, and native-feeling experience.

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop Shell | Tauri (Rust) |
| Frontend Framework | SvelteKit |
| Build Tool | Vite |
| Languages | Svelte, Rust, JavaScript, HTML |

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org)
- [Rust](https://www.rust-lang.org/tools/install)
- Tauri system dependencies for your platform (see the [Tauri prerequisites guide](https://tauri.app/start/prerequisites/))

### Installation

```bash
git clone https://github.com/Bonsa-Dereje/assista.git
cd assista
npm install
```

### Development

Run the app in development mode with hot reload:

```bash
npm run tauri dev
```

### Build

Create a production build:

```bash
npm run tauri build
```

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/)
- [Svelte for VS Code](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode)
- [Tauri for VS Code](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## License

MIT License