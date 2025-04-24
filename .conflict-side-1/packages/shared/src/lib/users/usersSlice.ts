import { buildLoadableTable } from '$lib/redux/defaultSlices';
import { type LoadableUserIdByLogin, type LoadableUser } from '$lib/users/types';

export const userTable = buildLoadableTable<LoadableUser>('user');

export const userByLoginTable = buildLoadableTable<LoadableUserIdByLogin>('userByLogin');
