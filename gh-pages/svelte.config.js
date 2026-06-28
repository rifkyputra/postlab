import adapter from '@sveltejs/adapter-static';

export default {
	kit: {
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: '404.html'
		}),
		paths: {
			base: '/postlab'
		}
	}
};
