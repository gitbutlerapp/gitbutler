import reactQueryPlugin from "@tanstack/eslint-plugin-query";
import reactHooksPlugin from "eslint-plugin-react-hooks";
import { defineConfig } from "oxlint";

const renameRulePrefixes = (
	rules: Record<string, unknown> | undefined,
	fromPrefix: string,
	toPrefix: string,
) =>
	Object.fromEntries(
		Object.entries(rules ?? {}).map(([ruleName, ruleConfig]) => [
			ruleName.replace(`${fromPrefix}/`, `${toPrefix}/`),
			ruleConfig,
		]),
	);

export default defineConfig({
	jsPlugins: [
		// The builtin plugin isn't 1:1 at time of writing.
		{ name: "react-hooks-js", specifier: "eslint-plugin-react-hooks" },
		{ name: "@tanstack/query", specifier: "@tanstack/eslint-plugin-query" },
	],
	plugins: ["eslint", "jsx-a11y", "oxc", "react", "typescript", "unicorn"],
	rules: {
		"arrow-body-style": ["error", "as-needed"],
		curly: ["warn", "multi"],
		"default-param-last": "error",
		"no-console": "warn",
		"no-cond-assign": ["warn", "always"],
		"no-fallthrough": "error",
		"no-param-reassign": "error",
		// Default config has catch violations.
		"no-unused-vars": [
			"warn",
			{
				caughtErrorsIgnorePattern: "^_",
				// These are the defaults that are unset by diverging our config at all.
				argsIgnorePattern: "^_",
				varsIgnorePattern: "^_",
			},
		],
		"oxc/no-barrel-file": ["warn", { threshold: 0 }],
		"prefer-template": "warn",
		"react/button-has-type": "error",
		"react/jsx-boolean-value": "warn",
		"react/jsx-fragments": "warn",
		// Lots of false positives on sums.
		"react/jsx-key": "off",
		"react/jsx-no-useless-fragment": "warn",
		"react/no-array-index-key": "warn",
		"react/no-danger": "error",
		// Temporarily disabled during the POC phase
		// "react/only-export-components": "error",
		"react/self-closing-comp": "warn",
		"typescript/array-type": ["warn", { default: "generic" }],
		"typescript/await-thenable": "error",
		"typescript/ban-ts-comment": "warn",
		"typescript/no-explicit-any": "error",
		"typescript/no-floating-promises": "error",
		"typescript/no-inferrable-types": "warn",
		"typescript/no-misused-promises": "error",
		"typescript/no-misused-spread": "error",
		"typescript/no-non-null-assertion": "warn",
		"typescript/no-unnecessary-condition": "warn",
		"typescript/no-unnecessary-template-expression": "warn",
		"typescript/no-unsafe-argument": "error",
		"typescript/no-unsafe-assignment": "error",
		"typescript/no-unsafe-call": "error",
		"typescript/no-unsafe-member-access": "error",
		"typescript/no-unsafe-return": "error",
		"typescript/parameter-properties": "error",
		"typescript/restrict-plus-operands": [
			"error",
			{
				allowAny: false,
				allowBoolean: false,
				allowNullish: false,
				allowNumberAndString: false,
				allowRegExp: false,
			},
		],
		"typescript/restrict-template-expressions": [
			"warn",
			{
				allowAny: false,
				allowNullish: false,
				allowRegExp: false,
			},
		],
		// "always" flags for lack of await outside of async functions, unlike ESLint:
		//   https://github.com/oxc-project/oxc/issues/18452
		"typescript/return-await": ["error", "error-handling-correctness-only"],
		"typescript/strict-boolean-expressions": [
			"warn",
			{
				allowString: false,
				allowNumber: false,
				allowNullableBoolean: true,
			},
		],
		"typescript/unbound-method": "error",
		"unicorn/no-document-cookie": "warn",
		"unicorn/prefer-number-properties": "error",

		...renameRulePrefixes(
			reactHooksPlugin.configs.recommended.rules,
			// react-hooks is reserved by Oxlint.
			"react-hooks",
			"react-hooks-js",
		),
		...reactQueryPlugin.configs.recommended.rules,
	},
});
