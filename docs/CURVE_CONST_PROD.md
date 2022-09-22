Let us first demonstrate the case for providing liquidity in a 2-dimensional
pool and we will then proceed to generalize it:

Let’s assume for a pool of
SOL/USDT the user deposits 10 SOL and 50 USDT.

```
k = 10*50 = 500
```

Therefore the initial invariant is set to 500. Let’s assume that after a while
some trade occurs and the pool state becomes the following:

```
x = sol = 5
y = usdt = 100
```

Whereas at the beginning, the ratio was of

```
x/y = 10 / 50 = 0.2
```

after the trading the ratio became

```
x/y = 5 / 100 = 0.05
```

Now if a user wants to provide liquidity it will have to do so in the current
ratio. The new user will then provide the following tokens,

```
\Delta x_1
\Delta x_2
```

such that:

```math
\frac{x + \Delta x}{y + \Delta y} = \frac{x}{y}
```

# Swapping

The trade starts by
the trader declaring how much $`y`$ he will want to sell, defined by

```math
|\Delta y|.
```

To find the amount of $`x`$ the trader will receive, defined by

```math
|\Delta x|,
```

we compute the following:

```math
(x - |\Delta x|)(y + |\Delta y|) = k \iff |\Delta x| = x - \frac{k}{y+ |\Delta y|}
```

Whilst we can frame this exercise in 2-dimensions as a buying/selling the
base/quote or vice-versa, when thinking in the n-dimension space the concept of
quote and base becomes fuzzy. To avoid confusions we will distinguish them as
tokens bought and tokens sold.

Let’s look now at the n-dimensional case. In the n-dimensional case, the trader
will still want to buy token $`x_b`$ in the reserve (b stands for buy) whilst
selling token $`x_s`$ in the reserve (s stands for sell).

The amount of

```math
\Delta x_s
```

is therefore given and we want to find

```math
\Delta x_b.
```

We compute the following:

```math
(x_b - |\Delta x_b|)(x_s + |\Delta x_s|) \prod_{i=1; i\not\in \{b, s\}}^{n}x_i = k \iff |\Delta x_b| = x_b -\frac{k}{(x_s + |\Delta x_s|) \prod_{i=1; i\not\in \{b, s\}}^{n}x_i}
```
