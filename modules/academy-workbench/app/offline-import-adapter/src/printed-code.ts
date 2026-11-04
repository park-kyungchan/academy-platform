// Printed paper code converter mirroring the native D32 codec observed
// through academy_source_read_contracts (kind=printed_code).
// Alphabet: 2-9 A-Z without 0/1/I/O (32 symbols). Codes carry a '#' prefix;
// the key part is 1-11 symbols. Pure offline function: no source access.
const ALPHABET='23456789ABCDEFGHJKLMNPQRSTUVWXYZ';
const INDEX=new Map([...ALPHABET].map((c,i)=>[c,i]));
const MAX_KEY_PART_LENGTH=11;
const MAX_SAFE_NUMBER=4294967295;
function fail(condition:unknown,reason:string):asserts condition{if(!condition)throw Error(reason);}
export function printedCodeToNumber(code:unknown):number{
 fail(typeof code==='string'&&code.length>1&&code.startsWith('#'),'invalid_printed_code');
 const keyPart=code.slice(1).toUpperCase();
 fail(keyPart.length>=1&&keyPart.length<=MAX_KEY_PART_LENGTH,'invalid_printed_code');
 let value=0;
 for(const char of keyPart){const digit=INDEX.get(char);fail(digit!==undefined,'invalid_printed_code');value=value*32+digit;}
 return value;
}
export function numberToPrintedCode(value:unknown):string{
 fail(typeof value==='number'&&Number.isInteger(value)&&value>=1&&value<=MAX_SAFE_NUMBER,'invalid_paper_number');
 let rest=value as number,result='';
 while(rest>0){result=ALPHABET[rest%32]+result;rest=Math.floor(rest/32);}
 return'#'+result;
}
