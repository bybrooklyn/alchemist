# Alchemist Documentation Site

This is the documentation site for Alchemist, built with [Astro Starlight](https://starlight.astro.build/).

## Getting Started

1. Install dependencies:
   ```bash
   npm install
   ```

2. Start the development server:
   ```bash
   npm run dev
   ```

3. Open [http://localhost:4321/alchemist-docs/](http://localhost:4321/alchemist-docs/) in your browser.

## Building and Deploying

To build the static site:
```bash
npm run build
```

The output will be in the `dist/` directory.

## Structure

- `src/content/docs/`: Markdown and MDX documentation files.
- `astro.config.mjs`: Starlight and Astro configuration.
- `public/`: Static assets (images, etc.).
