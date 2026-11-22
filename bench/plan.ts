// The order runs happen in.

/**
 * Every setup on every task, `reps` times. Setups take turns going first: on each task the order is
 * rotated by one, so no setup always runs right after the others have used a server (and met its rate
 * limits), which once made the setup that always ran last look slow.
 */
export const planOf = <S, T>(reps: number, tasks: ReadonlyArray<T>, setups: ReadonlyArray<S>) =>
  Array.from({ length: reps }, (_, i) => i + 1).flatMap((rep) =>
    tasks.flatMap((task, t) => {
      const shift = ((rep - 1) * tasks.length + t) % Math.max(setups.length, 1);
      const order = [...setups.slice(shift), ...setups.slice(0, shift)];
      return order.map((setup) => ({ setup, task, rep }));
    })
  );
