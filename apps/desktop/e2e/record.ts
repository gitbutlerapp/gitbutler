import path from 'node:path';
import { spawn } from 'child_process';
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
	ffmpeg: any;
	test!: Frameworks.Test;
	videoPath!: string;

	constructor() {}

	setPath(path: string) {
		this.videoPath = path;
	}

	stop() {
		this.ffmpeg?.kill('SIGINT');
	}

	start(test: Frameworks.Test) {
		this.test = test;

		// Throw error if video path not set
		if (!this.videoPath) {
			throw new Error('Video path not set. Set using setPath() function.');
		}

		if (process.env.DISPLAY && process.env.DISPLAY.startsWith(':')) {
			const videoPath = filePath({
				test: this.test,
				videoPath: this.videoPath,
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
				videoPath // Output file
			]);

			const logBuffer = function (buffer: Buffer, prefix: string) {
				const lines = buffer.toString().trim().split('\n');
				lines.forEach(function (line) {
					console.log(prefix + line);
				});
			};

			this.ffmpeg.stdout.on('data', (data: Buffer) => {
				logBuffer(data, 'ffmpeg stdout: ');
			});

			this.ffmpeg.stderr.on('data', (data: Buffer) => {
				logBuffer(data, 'ffmpeg stderr: ');
			});

			this.ffmpeg.on('close', (code: number, signal: string | unknown) => {
				if (code !== null) {
					console.log(`\tffmpeg exited with code ${code} ${videoPath}`);
				}
				if (signal !== null) {
					console.log(`\tffmpeg received signal ${signal} ${videoPath}`);
				}
			});
		}
	}
}
