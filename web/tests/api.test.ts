import { describe, it, expect, vi, beforeEach } from 'vitest';
import { api, ApiError } from '$lib/api/client';

beforeEach(() => {
	vi.restoreAllMocks();
});

describe('api client', () => {
	it('returns parsed JSON on success', async () => {
		global.fetch = vi.fn().mockResolvedValue({
			ok: true,
			json: () => Promise.resolve({ servers: [] }),
		} as Response);

		const result = await api.get<{ servers: unknown[] }>('/servers');
		expect(result.servers).toEqual([]);
	});

	it('throws ApiError on non-ok response', async () => {
		global.fetch = vi.fn().mockResolvedValue({
			ok: false,
			status: 404,
			statusText: 'Not Found',
			json: () => Promise.resolve({ error: 'server not found' }),
		} as Response);

		await expect(api.get('/servers/bad-id')).rejects.toThrow(ApiError);
	});
});
