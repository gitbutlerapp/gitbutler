import { router } from '@/router';
import { RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

const rootElement = document.getElementById('root');
if (!rootElement) {
	throw new Error('Root element not found');
}

const root = createRoot(rootElement);
root.render(
	<StrictMode>
		<RouterProvider router={router} />
	</StrictMode>
);
