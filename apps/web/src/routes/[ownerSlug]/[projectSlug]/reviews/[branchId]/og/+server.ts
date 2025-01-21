import Card from './Card.svelte';
import { HttpClient } from '@gitbutler/shared/network/httpClient';
import { gravatarUrlFromEmail } from '@gitbutler/ui/avatar/gravatar';
import { Resvg } from '@resvg/resvg-js';
import satori from 'satori';
import { html as satoriHtml } from 'satori-html';
import { render } from 'svelte/server';
import { readable } from 'svelte/store';
import { readFileSync } from 'fs';
import path from 'path';
import type { RequestHandler } from './$types';
import type { ApiBranch } from '@gitbutler/shared/branches/types';
import type { ApiUser } from '@gitbutler/shared/users/types';
import { env } from '$env/dynamic/public';

// Load the font data
const fontFilePath = path.resolve('src/lib/fonts/NotoSans-Regular.ttf');
const fontData = readFileSync(fontFilePath);

// eslint-disable-next-line func-style
export const GET: RequestHandler = async ({ params }) => {
	const httpClient = new HttpClient(fetch, env.PUBLIC_APP_HOST, readable(undefined));

	let branch: ApiBranch;
	let date: string;

	try {
		branch = await httpClient.get<ApiBranch>(
			`patch_stack/${params.ownerSlug}/${params.projectSlug}/branch/${params.branchId}`
		);
		const dateString = branch.created_at;
		const dateObj = new Date(dateString);
		// format date as Jun 30, 2021
		date = dateObj.toLocaleDateString('en-US', {
			year: 'numeric',
			month: 'short',
			day: 'numeric'
		});
	} catch (e) {
		console.error(e);
		return new Response();
	}

	let user: ApiUser | undefined;
	try {
		user = await httpClient.get<ApiUser>(`user/${branch.owner_login}`);
	} catch (_) {
		/* empty */
	}

	const slug = params.ownerSlug + '/' + params.projectSlug;
	const title = branch.title || 'unknown';
	const picture =
		user?.picture || (await gravatarUrlFromEmail(user?.email || 'example@example.com'));

	const commit_titles = branch.patches.map((patch) => patch.title || '') || [];
	const commits = branch.stack_size || 1;
	const files = branch.patches.reduce((acc, patch) => acc + patch.statistics.file_count, 0);
	const additions = branch.patches.reduce(
		(acc, patch) => acc + (patch.statistics.lines - patch.statistics.deletions),
		0
	);
	const subtractions = branch.patches.reduce((acc, patch) => acc + patch.statistics.deletions, 0);

	const { body } = render(Card, {
		props: { title, slug, picture, date, commit_titles, commits, files, additions, subtractions }
	});

	// Convert HTML string to VDOM
	const vdom = satoriHtml(body);

	// Generate the SVG using Satori
	// @ts-expect-error The satori-html library is fine
	const svg = await satori(vdom, {
		width: 1200,
		height: 630,
		fonts: [
			{
				name: 'Noto Sans',
				data: fontData,
				weight: 400,
				style: 'normal'
			}
		]
	});

	// Render the SVG to PNG using Resvg
	const resvg = new Resvg(svg);
	const pngData = resvg.render();

	return new Response(pngData.asPng(), {
		headers: { 'Content-Type': 'image/png' }
	});
};
