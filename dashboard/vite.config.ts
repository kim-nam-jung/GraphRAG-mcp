/// <reference types="vitest" />
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react({
    // @ts-expect-error: Babel config is valid but TS def might be outdated
    babel: {
      plugins: [
        ["babel-plugin-react-compiler", {}],
      ],
    },
  })],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/setupTests.tsx'],
    exclude: ['tests/e2e/**', 'node_modules/**', 'dist/**'],
  },
})
