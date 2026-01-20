# Rust in Everything

Rust in Everything is a comprehensive full-stack application built with [Dioxus 0.7](https://dioxuslabs.com/), showcasing the power of Rust in various domains including Web, Wasm, AI, and more.

## Tech Stack

- **Frontend/Fullstack**: Dioxus 0.7
- **Styling**: Tailwind CSS v4.0+
- **Router**: Dioxus Router

## Modules

### Blog Details
- **Description**: A fully functional blog system rendering Markdown content.
- **Features**:
  - Blog index page with article listing.
  - Detailed blog view with Markdown rendering.
  - Navigation between articles (Previous/Next).
  - Server-side content fetching (`get_blog_content`).

### Podcast
- **Description**: An immersive podcast player experience.
- **Features**:
  - Episode listing with duration and date.
  - Interactive audio player with play/pause controls.
  - Episode selection and playback.
  - Responsive design with a modern UI.

## Getting Started

1.  **Install Prerequisites**:
    -   Rust toolchain
    -   Dioxus CLI: `cargo install dioxus-cli`

2.  **Run the Application**:
    ```bash
    dx serve
    ```

## Development

This project follows a modular architecture.
- `src/components`: Reusable UI components.
- `src/routes`: Route definitions and page components.
- `src/server`: Server-side logic and functions.

## Documentation

- See `tailwind.md` for Tailwind CSS best practices and rules used in this project.
