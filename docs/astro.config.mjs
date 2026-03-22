import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	base: '/alchemist-docs/',
	integrations: [
		starlight({
			title: 'Alchemist Docs',
			social: {
				github: 'https://github.com/alchemist-project/alchemist',
			},
			sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'Installation', link: '/getting-started/installation/' },
						{ label: 'First Run', link: '/getting-started/first-run/' },
						{ label: 'Quick Start', link: '/getting-started/quick-start/' },
					],
				},
				{
					label: 'Guides',
					items: [
						{ label: 'Docker', link: '/guides/docker/' },
						{ label: 'Hardware Support', link: '/guides/hardware/' },
						{ label: 'GPU Passthrough', link: '/guides/gpu-passthrough/' },
						{ label: 'Web Interface', link: '/guides/web-interface/' },
						{ label: 'Library Setup', link: '/guides/library-setup/' },
						{ label: 'Profiles', link: '/guides/profiles/' },
						{ label: 'Streaming Rules', link: '/guides/stream-rules/' },
						{ label: 'Scheduling', link: '/guides/scheduling/' },
						{ label: 'Notifications', link: '/guides/notifications/' },
						{ label: 'Library Doctor', link: '/guides/library-doctor/' },
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'Configuration', link: '/reference/configuration/' },
						{ label: 'API Reference', link: '/reference/api/' },
						{ label: 'Database Schema', link: '/reference/database/' },
						{ label: 'Architecture', link: '/reference/architecture/' },
						{ label: 'Codecs', link: '/reference/codecs/' },
						{ label: 'Hardware Vendors', link: '/reference/hardware-support/' },
						{ label: 'FAQ', link: '/reference/faq/' },
						{ label: 'Troubleshooting', link: '/reference/troubleshooting/' },
						{ label: 'Changelog', link: '/reference/changelog/' },
					],
				},
				{
					label: 'Contributing',
					items: [
						{ label: 'Overview', link: '/contributing/overview/' },
						{ label: 'Development', link: '/contributing/development/' },
					],
				},
			],
		}),
	],
});
