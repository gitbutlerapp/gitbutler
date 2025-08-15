/**
 * @see https://prettier.io/docs/en/configuration.html
 * @type {import("prettier").Config}
 */
const config = {
  semi: false,
  singleQuote: false,
  trailingComma: "none",
  printWidth: 100,
  endOfLine: "auto",
  plugins: ["prettier-plugin-tailwindcss"]
}

export default config
