import { createMutation, useQueryClient } from '@tanstack/svelte-query';
import { api } from './client';

export interface CreateServerInput {
	name: string;
	host: string;
	port?: number;
	user?: string;
	auth_method?: string;
	ssh_key_path?: string;
}

export const createAddServerMutation = () => {
	const client = useQueryClient();
	return createMutation({
		mutationFn: (input: CreateServerInput) => api.post('/servers', input),
		onSuccess: () => client.invalidateQueries({ queryKey: ['servers'] }),
	});
};

export const createDeleteServerMutation = () => {
	const client = useQueryClient();
	return createMutation({
		mutationFn: (id: string) => api.delete(`/servers/${id}`),
		onSuccess: () => client.invalidateQueries({ queryKey: ['servers'] }),
	});
};

export const createInstallAppMutation = () => {
	const client = useQueryClient();
	return createMutation({
		mutationFn: ({ serverId, app }: { serverId: string; app: string }) =>
			api.post(`/servers/${serverId}/install`, { app }),
		onSuccess: () => client.invalidateQueries({ queryKey: ['tasks'] }),
	});
};

export const createUpgradeMutation = () => {
	const client = useQueryClient();
	return createMutation({
		mutationFn: (serverId: string) => api.post(`/servers/${serverId}/upgrade`, {}),
		onSuccess: () => client.invalidateQueries({ queryKey: ['tasks'] }),
	});
};

export const createHardenMutation = () => {
	const client = useQueryClient();
	return createMutation({
		mutationFn: (serverId: string) => api.post(`/servers/${serverId}/harden`, {}),
		onSuccess: () => client.invalidateQueries({ queryKey: ['tasks'] }),
	});
};
