/**
 * TS Configs... I hate them.
 *
 * TS Configs can point to other configs in two way:
 *
 * - `extends`, which is a "super" config that you need to fall back on.
 *      An important note about "super" configs, is that the paths are relative
 *      to the folder that the "super" config is in. OR the baseUrl.
 * - `references`, which lets you provide an array of TS Configs or directories
 *      that include TS Configs.
 *      These configs have different file includes and excludes.
 */
