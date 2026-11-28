/** The one builder of the option words of every ssh and scp this server spawns (the native pool bridge and the
 * PC executor of specs/academy-mcp/pc-executor.md). The host key is always checked (`StrictHostKeyChecking=yes`,
 * never `accept-new`) and no password or passphrase is asked (`BatchMode=yes`). With an identity file the words
 * are `-i <identityFile>` first and `IdentitiesOnly=yes` after the known-hosts file; `noConfig` adds `-F none`, so
 * the user's `~/.ssh/config` never applies. Without the optional members the words are those the native pool
 * bridge has always sent: `-o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=<seconds>`. */
export interface SshOptionWords {connectTimeoutSeconds:number;identityFile?:string;knownHostsFile?:string;noConfig?:boolean}
export function sshOptionArgv(options:SshOptionWords):string[]{
  if(!Number.isInteger(options.connectTimeoutSeconds)||options.connectTimeoutSeconds<1||options.connectTimeoutSeconds>120)throw Error('ssh_arguments_invalid');
  return [
    ...(options.identityFile!==undefined?['-i',options.identityFile]:[]),
    '-o','BatchMode=yes','-o','StrictHostKeyChecking=yes',
    ...(options.knownHostsFile!==undefined?['-o','UserKnownHostsFile='+options.knownHostsFile]:[]),
    ...(options.identityFile!==undefined?['-o','IdentitiesOnly=yes']:[]),
    '-o','ConnectTimeout='+options.connectTimeoutSeconds,
    ...(options.noConfig?['-F','none']:[]),
  ];
}
