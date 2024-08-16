import path from 'node:path';
import { spawn, type ChildProcessWithoutNullStreams } from 'child_process';
import type { Frameworks } from '@wdio/types';

function filePath({
	test,
	videoPath,
	extension
}: {
	test: Frameworks.Test;
	videoPath: string;
	extension: string;
}) {
	return path.join(videoPath, `${fileName(test.parent)}-${fileName(test.title)}.${extension}`);
}

function fileName(title: string) {
	return encodeURIComponent(title.trim().replace(/\s+/g, '-'));
}

export class TestRecorder {
	ffmpeg!: ChildProcessWithoutNullStreams;

	constructor() {}

	stop() {
		this.ffmpeg?.kill('SIGINT');
	}

	start(test: Frameworks.Test, videoPath: string) {
		if (!videoPath) {
			throw new Error('Video path not set. Set using setPath() function.');
		}

		if (process.env.DISPLAY && process.env.DISPLAY.startsWith(':')) {
			const parsedPath = filePath({
				test,
				videoPath,
				extension: 'mp4'
			});

			this.ffmpeg = spawn('ffmpeg', [
				'-f',
				'x11grab', //  Grab the X11 display
				'-video_size',
				'1280x1024', // Video size
				'-i',
				process.env.DISPLAY, // Input file url
				'-loglevel',
				'error', // Log only errors
				'-y', // Overwrite output files without asking
				'-pix_fmt',
				'yuv420p', // QuickTime Player support, "Use -pix_fmt yuv420p for compatibility with outdated media players"
				parsedPath // Output file
			]);

			const logBuffer = function (buffer: Buffer, prefix: string) {
				const lines = buffer.toString().trim().split('\n');
				lines.forEach(function (line) {
					console.log(prefix + line);
				});
			};

			this.ffmpeg.stdout.on('data', (data: Buffer) => {
				logBuffer(data, '[ffmpeg:stdout] ');
			});

			this.ffmpeg.stderr.on('data', (data: Buffer) => {
				logBuffer(data, '[ffmpeg:error] ');
			});

			this.ffmpeg.on('close', (code: number, signal: string | unknown) => {
				if (code !== null) {
					console.log(`[ffmpeg:stdout] exited with code ${code}: ${videoPath}`);
				}
				if (signal !== null) {
					console.log(`[ffmpeg:stdout] received signal ${signal}: ${videoPath}`);
				}
			});
		}
	}
}
