/** @type {import("eslint").Linter.Config} */
const config = {
  parser: '@typescript-eslint/parser',
  parserOptions: {
    sourceType: 'module',
    ecmaVersion: '2023'
  },
  extends: [
    'eslint:recommended',
    'next',
    'plugin:@typescript-eslint/eslint-recommended',
    'plugin:@typescript-eslint/recommended'
  ],
  env: {
    browser: true,
    es6: true
  },
  plugins: ['prettier', '@typescript-eslint'],
  rules: {
    '@next/next/no-img-element': 'off'
  }
}

module.exports = config
