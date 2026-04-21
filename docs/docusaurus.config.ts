import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const url = process.env.DOCS_URL ?? 'https://alchemist-project.org';
const baseUrl = process.env.DOCS_BASE_URL ?? '/';

const config: Config = {
  title: 'Alchemist',
  tagline: 'Self-hosted video transcoding automation. Point it at your library. Walk away.',

  future: {
    v4: true,
  },

  url,
  baseUrl,

  organizationName: 'bybrooklyn',
  projectName: 'alchemist',

  onBrokenLinks: 'throw',
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },

  headTags: [
    {
      tagName: 'script',
      attributes: {
        type: 'application/ld+json',
      },
      innerHTML: JSON.stringify({
        '@context': 'https://schema.org',
        '@type': 'SoftwareApplication',
        name: 'Alchemist',
        description:
          'Self-hosted video transcoding automation. Scans your media library, analyzes every file, and encodes only what is worth encoding using NVENC, Intel Quick Sync, VAAPI, AMD AMF, or Apple VideoToolbox. GPLv3.',
        applicationCategory: 'MultimediaApplication',
        operatingSystem: 'Linux, macOS, Windows',
        url,
        license: 'https://www.gnu.org/licenses/gpl-3.0.html',
        offers: {
          '@type': 'Offer',
          price: '0',
          priceCurrency: 'USD',
        },
        sameAs: ['https://github.com/bybrooklyn/alchemist'],
      }),
    },
  ],

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          routeBasePath: '/',
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/bybrooklyn/alchemist/edit/main/docs/',
        },
        pages: false,
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: 'img/social-card.png',
    docs: {
      sidebar: {
        hideable: false,
        autoCollapseCategories: false,
      },
    },
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: true,
      respectPrefersColorScheme: false,
    },
    navbar: {
      title: 'Alchemist',
      items: [
        {
          type: 'doc',
          docId: 'overview',
          position: 'left',
          label: 'Overview',
        },
        {
          type: 'doc',
          docId: 'installation',
          position: 'left',
          label: 'Install',
        },
        {
          type: 'doc',
          docId: 'hardware',
          position: 'left',
          label: 'Hardware',
        },
        {
          type: 'doc',
          docId: 'configuration-reference',
          position: 'left',
          label: 'Reference',
        },
        {
          href: 'https://github.com/bybrooklyn/alchemist',
          label: 'GitHub',
          position: 'right',
        },
        {
          href: 'https://github.com/bybrooklyn/alchemist/releases',
          label: 'Releases',
          position: 'right',
          className: 'navbar-releases-button',
        },
      ],
    },
    footer: {
      links: [
        {
          title: 'Get Started',
          items: [
            {label: 'Overview', to: '/'},
            {label: 'Installation', to: '/installation'},
            {label: 'First Run', to: '/first-run'},
            {label: 'Quick Start', to: '/quick-start'},
            {label: 'Docker', to: '/docker'},
          ],
        },
        {
          title: 'Guides',
          items: [
            {label: 'Hardware Acceleration', to: '/hardware'},
            {label: 'Library Setup', to: '/library-setup'},
            {label: 'Profiles', to: '/profiles'},
            {label: 'Stream Rules', to: '/stream-rules'},
            {label: 'Notifications', to: '/notifications'},
          ],
        },
        {
          title: 'Reference',
          items: [
            {label: 'Configuration', to: '/configuration-reference'},
            {label: 'Skip Decisions', to: '/skip-decisions'},
            {label: 'Engine Modes', to: '/engine-modes'},
            {label: 'Environment Variables', to: '/environment-variables'},
            {label: 'API', to: '/api'},
            {label: 'Changelog', to: '/changelog'},
          ],
        },
        {
          title: 'Project',
          items: [
            {
              label: 'GitHub',
              href: 'https://github.com/bybrooklyn/alchemist',
            },
            {
              label: 'Releases',
              href: 'https://github.com/bybrooklyn/alchemist/releases',
            },
            {
              label: 'Issues',
              href: 'https://github.com/bybrooklyn/alchemist/issues',
            },
            {
              label: 'GPLv3 License',
              href: 'https://github.com/bybrooklyn/alchemist/blob/main/LICENSE',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Brooklyn Halmstad. Alchemist is GPLv3 open source.`,
    },
    prism: {
      theme: prismThemes.dracula,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
