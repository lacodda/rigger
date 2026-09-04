// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	site: 'https://lacodda.github.io',
	base: '/rigger',
	integrations: [
		starlight({
			title: 'rigger',
			description: 'One seat for all your projects and tasks: a local record of what is done, what is next and when it ships - read by you and your coding assistant.',
			logo: {
				src: './src/assets/logo.svg',
				alt: 'rigger',
			},
			favicon: '/favicon.svg',
			customCss: ['./src/styles/brand.css'],
			head: [
				{ tag: 'link', attrs: { rel: 'apple-touch-icon', href: '/rigger/apple-touch-icon.png' } },
				{ tag: 'meta', attrs: { property: 'og:image', content: 'https://raw.githubusercontent.com/lacodda/rigger/main/assets/social-preview.png' } },
				{ tag: 'meta', attrs: { name: 'twitter:card', content: 'summary_large_image' } },
			],
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/lacodda/rigger' }],
			editLink: {
				baseUrl: 'https://github.com/lacodda/rigger/edit/main/docs/',
			},
			sidebar: [
				{ label: 'Getting Started', slug: 'getting-started' },
				{
					label: 'Concepts',
					items: [{ autogenerate: { directory: 'concepts' } }],
				},
				{
					label: 'Reference',
					items: [{ autogenerate: { directory: 'reference' } }],
				},
			],
		}),
	],
});
