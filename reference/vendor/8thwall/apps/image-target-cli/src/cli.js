import {createInterface} from 'readline'

/**
 * @returns {import("./types").CliInterface}
 */
const createCliInterface = () => {
  const rl = createInterface({
    input: process.stdin,
    output: process.stdout,
  })
  rl.pause()

  const lines = []

  const waiting = []

  rl.on('line', (line) => {
    if (waiting.length) {
      waiting[0](line)
      waiting.shift()
    } else {
      lines.push(line)
    }
    rl.pause()
  })
  const close = () => {
    rl.close()
  }

  /**
   * @param {string} question
   * @returns {Promise<string>}
   */
  const prompt = async (question) => {
    console.log(question)
    if (lines.length) {
      return lines.shift()
    } else {
      rl.resume()
      return new Promise((resolve) => {
        waiting.push(resolve)
      })
    }
  }

  const choose = async (question, options, firstIsDefault) => {
    const optionStr = options
      .map(
        (opt, index) => `${index + 1}) ${opt} ${index === 0 && firstIsDefault ? '(default)' : ''}`
      )
      .join('\n')

    // eslint-disable-next-line no-constant-condition
    while (true) {
      const answer = await prompt(`${question}\n${optionStr}\n`)
      const normalized = answer.trim()

      if (normalized === '' && firstIsDefault) {
        return options[0]
      }

      const choiceIndex = parseInt(normalized, 10) - 1
      if (choiceIndex >= 0 && choiceIndex < options.length) {
        return options[choiceIndex]
      } else if (options.includes(normalized)) {
        return normalized
      } else {
        console.log('Invalid choice. Please try again.')
      }
    }
  }

  const confirm = async (question, defaultValue) => {
    const defaultStr = defaultValue ? '[Y/n]' : '[y/N]'

    // eslint-disable-next-line no-constant-condition
    while (true) {
      const answer = await prompt(`${question} ${defaultStr}: `)
      const normalized = answer.trim().toLowerCase()
      if (normalized === 'y' || normalized === 'yes') {
        return true
      } else if (normalized === 'n' || normalized === 'no') {
        return false
      } else if (normalized === '' && defaultValue !== undefined) {
        return defaultValue
      } else {
        console.log('Invalid input. Please enter y/n.')
      }
    }
  }

  const promptInteger = async (question) => {
    // eslint-disable-next-line no-constant-condition
    while (true) {
      const answer = await prompt(question)
      const parsed = Number(answer.trim())
      if (Number.isNaN(parsed) || !Number.isInteger(parsed)) {
        console.log('Invalid input. Please enter an integer.')
      } else {
        return parsed
      }
    }
  }

  const promptFloat = async (question) => {
    // eslint-disable-next-line no-constant-condition
    while (true) {
      const answer = await prompt(question)
      const parsed = Number(answer.trim())
      if (Number.isNaN(parsed)) {
        console.log('Invalid input. Please enter a number.')
      } else {
        return parsed
      }
    }
  }

  return {
    prompt,
    choose,
    close,
    promptInteger,
    promptFloat,
    confirm,
  }
}

export {createCliInterface}
