import { createPreset } from "fumadocs-ui/tailwind-plugin"

/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./components/**/*.{ts,tsx}",
    "./app/**/*.{ts,tsx}",
    "./content/**/*.{md,mdx}",
    "./mdx-components.{ts,tsx}",
    "./node_modules/fumadocs-ui/dist/**/*.js",
    "./node_modules/fumadocs-openapi/dist/**/*.js"
  ],
  theme: {
    extend: {
      fontFamily: {
        sans: ["var(--font-inter)", "var(--font-system)"],
        ppeditorial: ["var(--font-ppeditorialnew)"]
      }
    }
  },
  presets: [createPreset()]
}
