import { type BaseLayoutProps, type DocsLayoutProps } from 'fumadocs-ui/layout';
import { pageTree } from './source';
import Logo from '../components/Logo';

// shared configuration
export const baseOptions: BaseLayoutProps = {
  nav: {
    component: <Logo />,
    transparentMode: 'top',
  },
  githubUrl: 'https://github.com/gitbutlerapp/gitbutler',
  links: [
    {
      text: 'Documentation',
      url: '/docs',
      active: 'nested-url',
    },
  ],
};

// docs layout configuration
export const docsOptions: DocsLayoutProps = {
  ...baseOptions,
  tree: pageTree,
};
