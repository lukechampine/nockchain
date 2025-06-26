|=  [name=@t p=inputs:transact]
~&  "draft: {<name>}"
=/  inputs  `(list [nname:transact input:transact])`~(tap z-by p)
=/  by-addrs
  %+  roll  inputs
  |=  [[name=nname:transact input=input:transact] acc=_`(z-map lock:transact coins:transact)`~]
  =/  seeds  ~(tap z-in seeds:spend:input)
  %+  roll  seeds
  |=  [seed=seed:transact acc=_acc]
  =/  lock  recipient:seed
  =/  cur  (~(gut z-by acc) lock 0)
  =/  gift  gift:seed
  =/  new-bal  (add cur gift)
  (~(put z-by acc) lock new-bal)
=/  bals
  %+  roll  ~(tap z-by by-addrs)
  |=  [[recipient=lock:transact amt=coins:transact] acc=_`(list [[m=@udD pks=(list @t)] [@ud @ud]])`~]
  =/  r58  (to-b58:lock:transact recipient)
  =/  amtdiv  (dvr amt 65.536)
  ~&  "{<p.amtdiv>} nocks and {<q.amtdiv>} nicks to {<r58>}"
  [[r58 amtdiv] acc]
~
