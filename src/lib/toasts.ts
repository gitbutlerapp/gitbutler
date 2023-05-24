import toast, { type ToastOptions, type ToastPosition } from 'svelte-french-toast';
export { Toaster } from 'svelte-french-toast';

const defaultOptions = {
	position: 'bottom-center' as ToastPosition,
	style: 'border-radius: 8px; background: black; color: #fff;'
};

export const error = (msg: string, options: ToastOptions = {}) =>
	toast.error(msg, { ...defaultOptions, ...options });

export const success = (msg: string, options: ToastOptions = {}) =>
	toast.success(msg, { ...defaultOptions, ...options });

export const promise = (
	promise: Promise<any>,
	opts: { loading: string; success: string; error: string } = {
		loading: 'Loading...',
		success: 'Success!',
		error: 'Error!'
	}
) => toast.promise(promise, opts, defaultOptions);
