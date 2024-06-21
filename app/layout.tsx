import './global.css';
import { RootProvider } from 'fumadocs-ui/provider';
import { Inter } from 'next/font/google';
import localFont from 'next/font/local'
import type { ReactNode } from 'react';

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

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className={`${inter.className} ${splineSans.className} ${ppEditorialNew.className}`} suppressHydrationWarning>
      <body>
        <RootProvider>{children}</RootProvider>
      </body>
    </html>
  );
}
