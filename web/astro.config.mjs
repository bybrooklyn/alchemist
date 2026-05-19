import { fileURLToPath } from 'node:url';
import { defineConfig } from 'astro/config';
import react from '@astrojs/react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  integrations: [react()],
  vite: {
    plugins: [tailwindcss()],
    resolve: {
      alias: {
        'eventemitter3': fileURLToPath(new URL('./src/vendor/eventemitter3.ts', import.meta.url)),
        'prop-types': fileURLToPath(new URL('./src/vendor/prop-types.ts', import.meta.url)),
        'react-transition-group': fileURLToPath(new URL('./src/vendor/react-transition-group.tsx', import.meta.url)),
        'tiny-invariant': fileURLToPath(new URL('./src/vendor/tiny-invariant.ts', import.meta.url)),
      },
    },
  },
});
