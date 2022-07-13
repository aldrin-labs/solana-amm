Let us first look at the 2-dimensional case. Looking at the constant product
curve and constant sum curve:

```math
x + y = D
```

```math
xy=k
```

In a perfectly balanced pool, the amount of token $`x`$ will be the same amount
as token $`y`$. Therefore the following equality holds:

```math
\left( \frac {D}{n} \right)^n = k
```

Where $`n`$ represents the number of tokens in pools, or dimension of the pool,
which in this case is two.

The stable swap essentially adds these two curves together and uses a parameter
$`\chi`$ to control the relevance of each of the curves in the overall stable
curve.

```math
x + y + xy = D + k \\\iff \\
x + y +xy = D + \left( \frac {D}{n} \right)^n
```

---

Since the constant product curve is of dimension $`n`$ and the sum curve is of
dimension $`1`$ and since we want these to be at the same dimensionality we
multiply the sum curve by $`D^{n-1}`$:

```math
D^{n-1}\sum_{i=1}^{n}x_i + \prod_{i=1}^{n}x_i = D^n + \left( \frac {D}{n} \right)^n
```

Adding the parameter $`\chi`$ it becomes:

```math
D^{n-1} \chi \sum_{i=1}^{n}x_i + \prod_{i=1}^{n}x_i = D^n \chi + \left( \frac {d}{n} \right)^n
```

This means that when $`\chi = 0`$ the curve converges to the Constant Product
curve. In the

```math
\lim_{\chi \to\infty},
```

The Constant Product curve becomes irrelevant and the curve takes the shape of
the Constant Sum curve.

---

The key to do this is to remember that in an equally balanced constant product
curve, the following is true:

```math
\prod_{i=1}^{n}x_i = \left( \frac {D}{n} \right)^n
```

Whenever the pool becomes unequally balanced, the equality does not hold and
the following becomes true:

```math
\prod_{i=1}^{n}x_i < \left( \frac {D}{n} \right)^n
```

This inequality increases as the pool becomes more imbalanced, because the LHS
goes to zero whilst the RHS remains constant. Therefore we can make $`\chi`$
evanescent as the pool becomes increasingly imbalanced if we set:

```math
\chi = A \frac{\prod_{i=1}^{n}x_i}{\left( \frac {D}{n} \right)^n}
```

Where A is a parameter to adjust for scale. We therefore substitute $`\chi`$
for the term above and get:

```math
D^{n-1}A \frac{\prod_{i=1}^{n}x_i}{\left( \frac {D}{n} \right)^n} \sum_{i=1}^{n}x_i + \prod_{i=1}^{n}x_i = A \frac{\prod_{i=1}^{n}x_i}{\left( \frac {D}{n} \right)^n} D^n + \left( \frac {D}{n} \right)^n
```

# Providing liquidity

Let’s assume $`A = 20`$.

We start with a 2-dimension example, by inject 50 USDC and 50 DAI into the pool:

```math
0 = \frac{1}{2^2(50 \cdot 50)}D^{2+1} + D(2^2 \cdot 20 - 1) - 2^2 \cdot 20(50+50)
\\\iff \\
0 = \frac{1}{10000}D^{3} + 79D - 8000
```

If we assume the pool to have 1USDC and 99 DAI we get the following polynomial
instead:

```math
0 = \frac{1}{2^2(1 \cdot 99)}D^{2+1} + D(2^2 \cdot 20 - 1) - 2^2 \cdot 20(1+99)
\\\iff \\
0 = \frac{1}{396}D^{3} + 79D - 8000
```

The reality is that with or without balance the curve polynomial will always
have a linear shape with a single real positive root. This is because the
coefficient $`b`$ in $`bx`$ is overwhelmingly bigger than $`a`$ in $`ax^3`$.

Moving on, the Newton-Raphson algorithm is the following:

1. Start with an initial guess $`d_0`$

We will start with the guess which represents when the curve is at perfect
balance (d_0 is a guess on D).

```math
d_o = \sum_{i=1}^{n}x_i,
```

Assuming $`x_1 = 20`$ and $`x_2 = 40`$

```math
x_0 = 20 + 40 = 60
```

2. We compute $`f(d_0)`$ :

The general formula is

```math
f(d_0) = d_0^{n+1}\frac{1}{n^n\prod_{i=1}^{n}x_i} + d_0(n^nA -1) - An^n\sum_{i=1}^{n}x_i
```

3. We then compute the derivative $`f'(x_0)`$:

In general:

```math
f'(x) = (n+1) \cdot ax^{n} +  x
```

We apply this to our polynomial and get:

```math
f'(d_0) = d_0^{n}\frac{n+1}{n^n\prod_{i=1}^{n}x_i} + (n^nA -1)
```

4. We then compute $`d_1`$ by:

```math
d_{1} = d_0 - \frac{f(d_0)}{f'(d_0)}
```

Or in a more general case

```math
x_{n+1} = x_n - \frac{f(x_n)}{f'(x_n)}
```

5. We then repeat this process over and over again until the different between $`x_{n+1}`$ and $`x_n`$ is sufficiently close to zero.

# Swapping

Let $`x_b`$ be the the token amount of the token being bought, unknown at
start, and $`x_s`$ the token amount of the token being sold, known at start.
By algebraic manipulations, similar to those above, we arrive at a quadratic
polynomial on $`x_b`$

```math
0 = An^n \cdot x_b^2 - x_b \Big[D(n^nA-1) - An^n\sum_{i=1;i \neq i=b}^{n}x_i\Big]  - D^{n+1}\frac{1}{n^n\prod_{i=1; i\neq b}^{n}x_i}
\\ \iff \\
0 = x_b^2 - Bx_b  - C
```

Where both B and C are known. To find out what the amount of tokens the user is
eligible to buy, i.e. $`x_b`$, we can solve the equation with the quadratic
solution formula.
