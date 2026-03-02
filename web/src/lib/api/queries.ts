import { createQuery } from '@tanstack/svelte-query';
import { api } from './client';

export interface Server {
	id: string;
	name: string;
	host: string;
	port: number;
	user: string;
	auth_method: string;
	ssh_key_path: string | null;
	os_family: string | null;
	created_at: string;
	updated_at: string;
}

export interface Task {
	id: string;
	server_id: string;
	kind: string;
	status: 'pending' | 'running' | 'success' | 'failed';
	input_json: string | null;
	output: string | null;
	error: string | null;
	created_at: string;
	started_at: string | null;
	completed_at: string | null;
}

export const createServersQuery = () =>
	createQuery<{ servers: Server[] }>({
		queryKey: ['servers'],
		queryFn: () => api.get('/servers'),
	});

export const createServerQuery = (id: string) =>
	createQuery<Server>({
		queryKey: ['servers', id],
		queryFn: () => api.get(`/servers/${id}`),
	});

export const createTasksQuery = (serverId?: string) =>
	createQuery<{ tasks: Task[] }>({
		queryKey: ['tasks', serverId],
		queryFn: () => api.get(serverId ? `/tasks?server_id=${serverId}` : '/tasks'),
	});

export const createTaskQuery = (id: string) =>
	createQuery<Task>({
		queryKey: ['tasks', id],
		queryFn: () => api.get(`/tasks/${id}`),
	});
