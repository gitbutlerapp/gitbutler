# Toast Component

A simple toast notification system for the GitButler UI package.

## Features

- **Simple**: Just type and text - no complex configuration
- **Bottom-center positioning**: Appears at the bottom center of the screen
- **Stacking**: Multiple toasts stack vertically
- **Auto-dismiss**: All toasts automatically disappear after 4 seconds
- **4 types**: Info, Success, Warning, and Error with appropriate styling

## Usage

### Basic Usage

```svelte
<script>
	import { toasts, ToastContainer } from '@gitbutler/ui';

	function showToast() {
		toasts.success('Operation completed!');
	}
</script>

<button on:click={showToast}>Show Toast</button>

<!-- Add the container to your app root -->
<ToastContainer />
```

### Toast Types

```javascript
import { toasts } from '@gitbutler/ui';

// Different toast types
toasts.info('This is a info message');
toasts.success('Operation completed successfully!');
toasts.warning('Please review your changes');
toasts.error('Something went wrong');
```

### Promise Integration

```javascript
import { toasts } from '@gitbutler/ui';

async function handleAsyncOperation() {
	const myPromise = fetch('/api/data');

	await toasts.promise(myPromise, {
		loading: 'Loading data...',
		success: 'Data loaded successfully!',
		error: 'Failed to load data'
	});
}
```

### Clear All Toasts

```javascript
toasts.clearAll(); // Removes all current toasts
```

## Components

### `<Toast>`

Individual toast component.

**Props:**

- `type: 'info' | 'success' | 'warning' | 'error'` - Toast type
- `message: string` - Toast message text

### `<ToastContainer>`

Container that manages positioning and stacking. Add this once to your app root.
