import toast, { type ToastOptions, type ToastPosition } from 'svelte-french-toast';

const defaultOptions = {
	position: 'bottom-center' as ToastPosition,
	style: 'border-radius: 200px; background: #333; color: #fff;'
};

export const error = (msg: string, options: ToastOptions = {}) =>
	toast.error(msg, { ...defaultOptions, ...options });

export const success = (msg: string, options: ToastOptions = {}) =>
	toast.success(msg, { ...defaultOptions, ...options });
