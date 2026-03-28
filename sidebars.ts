import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'overview',
    {
      type: 'category',
      label: 'Get Started',
      collapsible: false,
      collapsed: false,
      items: ['installation', 'first-run', 'quick-start', 'docker'],
    },
    {
      type: 'category',
      label: 'Guides',
      collapsible: false,
      collapsed: false,
      items: [
        'hardware',
        'gpu-passthrough',
        'library-setup',
        'profiles',
        'stream-rules',
        'scheduling',
        'notifications',
        'library-doctor',
        'web-interface',
      ],
    },
    {
      type: 'category',
      label: 'Hardware',
      collapsible: false,
      collapsed: false,
      items: [
        'hardware/nvidia',
        'hardware/intel',
        'hardware/amd',
        'hardware/apple',
        'hardware/cpu',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      collapsible: false,
      collapsed: false,
      items: [
        'configuration-reference',
        'api',
        'codecs',
        'skip-decisions',
        'engine-modes',
        'environment-variables',
        'database-schema',
        'architecture',
        'troubleshooting',
        'faq',
        'changelog',
      ],
    },
    {
      type: 'category',
      label: 'Contributing',
      collapsible: false,
      collapsed: false,
      items: ['contributing/overview', 'contributing/development'],
    },
  ],
};

export default sidebars;
