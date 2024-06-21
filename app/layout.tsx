import './global.css';
import { RootProvider } from 'fumadocs-ui/provider';
import { Inter } from 'next/font/google';
  import localFont from 'next/font/local'
import type { Metadata, Viewport } from 'next'
import type { ReactNode } from 'react';

const SITE_URL = process.env.SITE_URL ?? 'http://localhost:3000'
const urlBase = new URL(SITE_URL)

const inter = Inter({
  subsets: ['latin'],
});

const splineSans = localFont({
  src: [
    {
      path: '../public/fonts/SplineSansMono-Regular.woff2',
      weight: '300'
    },
    {
      path: '../public/fonts/SplineSansMono-Medium.woff2',
      weight: '400'
    },
    {
      path: '../public/fonts/SplineSansMono-Semibold.woff2',
      weight: '600'
    }
  ],
  variable: '--font-splinesansmono'
})

const ppEditorialNew = localFont({
  src: [
    {
      path: '../public/fonts/PPEditorialNew-Regular.woff2',
      weight: '300'
    },
    {
      path: '../public/fonts/PPEditorialNew-Italic.woff2',
      weight: '300',
      style: 'italic'
    }
  ],
  variable: '--font-ppeditorialnew'
})



export const metadata: Metadata = {
	title: {
		template: 'GitButler - %s',
		default: 'GitButler',
	},
	description:
		'GitButler is a new Source Code Management system designed to manage your branches, record and backup your work, be your Git client, help with your code and much more',
	twitter: {
		card: 'summary_large_image',
	},
	openGraph: {
		images: '/cover.png',
		title: {
			template: 'GitButler - %s',
			default: 'GitButler',
		},
		description:
			'GitButler is a new Source Code Management system designed to manage your branches, record and backup your work, be your Git client, help with your code and much more',
	},
	metadataBase: urlBase,
	applicationName: 'GitButler',
	manifest: '/manifest.webmanifest',
	robots: {
		index: true,
		follow: true,
	},
	alternates: {
		types: {
			'application/rss+xml': [{ url: '/feed.xml', title: 'rss' }],
		},
	},
	icons: { icon: '/fav/fav-64.png', apple: '/fav/fav-180.png' },
}

export const viewport: Viewport = {
	themeColor: [
		{ media: '(prefers-color-scheme: dark)', color: '#000000' },
		{ media: '(prefers-color-scheme: light)', color: '#ffffff' },
	],
	colorScheme: 'dark light',
	width: 'device-width',
	initialScale: 1,
}

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className={`${inter.className} ${splineSans.className} ${ppEditorialNew.className}`} suppressHydrationWarning>
      <body>
        <RootProvider>{children}</RootProvider>
      </body>
    </html>
  );
}
