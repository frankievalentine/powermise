import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';
import starlight from '@astrojs/starlight';
import path from 'node:path';
import starlightThemeBlack from 'starlight-theme-black';

export default defineConfig({
  site: 'https://powermise.dev',
  vite: {
    build: {
      cssMinify: 'esbuild',
      chunkSizeWarningLimit: 500
    },
    resolve: {
      alias: {
        'starlight-theme-black/overrides/Sidebar.astro': path.resolve('src/starlight-theme-black/overrides/Sidebar.astro')
      }
    }
  },
  integrations: [
    sitemap(),
    starlight({
      title: 'powermise',
      description: 'A desktop companion app for mise-en-place.',
      favicon: './favicon.svg',
      disable404Route: true,
      customCss: ['./src/styles/docs.css'],
      plugins: [
        starlightThemeBlack({
          navLinks: [{ label: 'GitHub', link: 'https://github.com/frankievalentine/powermise' }],
          footerText: 'powermise v0.1.0'
        })
      ],
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/frankievalentine/powermise' }],
      editLink: {
        baseUrl: 'https://github.com/frankievalentine/powermise/edit/main/apps/web/'
      },
      lastUpdated: true,
      components: {
        Head: './src/components/DocHead.astro'
      },
      sidebar: [
        {
          label: 'Start here',
          items: ['what-is-powermise', 'getting-started', 'app-tour']
        },
        {
          label: 'How it works',
          items: ['how-it-works', 'troubleshooting']
        },
        {
          label: 'AI Setups',
          items: [
            'ai-setups',
            'ai-setups/getting-started',
            'ai-setups/harness-selections',
            'ai-setups/selection-schema',
            'ai-setups/catalog',
            'ai-setups/harness-reference',
            'ai-setups/targets',
            'ai-setups/commands'
          ]
        },
        {
          label: 'AI Setups guides',
          collapsed: true,
          items: [
            'ai-setups/mise-boundary',
            'ai-setups/lifecycle',
            'ai-setups/shell-integration',
            'ai-setups/automation',
            'ai-setups/safety',
            'ai-setups/coexistence',
            'ai-setups/chezmoi'
          ]
        }
      ]
    })
  ]
});
