# svelte-comment-injector

A Svelte preprocessor that injects an HTML comment at the top of every component, making it easier to identify components in browser DevTools.

## ✨ Features

- Injects a DevTools-visible HTML comment like:
  ```html
  <!-- Begin MyComponent.svelte -->
  <div class="my-component">
  	<!-- Component content -->
  </div>
  <!-- End MyComponent.svelte -->
  ```

## 📦 Installation

```bash
npm install @gitbutler/svelte-comment-injector --save-dev
```

## ⚙️ Usage

Add the preprocessor to your Svelte configuration:

```js
// svelte.config.js
import svelteInjectComment from "@gitbutler/svelte-comment-injector";

export default {
	preprocess: [svelteInjectComment()],
	// ...rest of your config
};
```

## 🛠️ Configuration

You can customize the comment format by passing options to the preprocessor:

```js
// svelte.config.js
import svelteInjectComment from "@gitbutler/svelte-comment-injector";

export default {
	preprocess: [
		svelteInjectComment({
			enabled: true, // Enable or disable the comment injection
			showEndComment: true, // Show the end comment
			showFullPath: false, // Show the full path in the comment
		}),
	],
	// ...rest of your config
};
```
