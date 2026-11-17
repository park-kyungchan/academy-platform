/** The DailyRecord review consults no learning catalog. An installed release may
 * still pass `--dailyrecord-learning <bundle> <sha> <config> <sha>`: those five
 * arguments are removed here, no file is read and nothing is gated. */
export function withoutRetiredLearningArguments(args:readonly string[]){
  const remainingArgs:string[]=[];
  for(let i=0;i<args.length;i++){
    if(args[i]!=='--dailyrecord-learning'){remainingArgs.push(args[i]!);continue;}
    if(i+4>=args.length)throw Error('dailyrecord_learning_arguments_invalid');
    i+=4;
  }
  return {remainingArgs};
}
