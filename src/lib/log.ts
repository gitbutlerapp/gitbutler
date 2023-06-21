import { building } from '$app/environment';

export async function setup() {
	if (!building) {
		await (await import('tauri-plugin-log-api')).attachConsole();
	}
}

const logger = async () =>
	building
		? {
				debug: () => {},
				info: () => {},
				error: () => {}
		  }
		: import('tauri-plugin-log-api').then((tauri) => ({
				debug: tauri.debug,
				info: tauri.info,
				error: tauri.error
		  }));

const toString = (value: any) => {
	if (value instanceof Error) {
		return value.message;
	} else if (typeof value === 'object') {
		return JSON.stringify(value);
	} else {
		return value.toString();
	}
};

export async function debug(...args: any[]) {
	return (await logger()).debug(args.map(toString).join(' '));
}

export async function info(...args: any[]) {
	return (await logger()).info(args.map(toString).join(' '));
}

export async function error(...args: any[]) {
	(await logger()).error(args.map(toString).join(' '));
}
