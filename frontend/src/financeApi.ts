import { config } from "./config";
import { authenticatedRequest, type ApiClientOptions } from "./apiCore";
import type {
  CreateFinanceExpenseRequest,
  CreateFinanceReceivableRequest,
  FinanceExpense,
  FinanceExpenseQuery,
  FinanceReceivable,
  FinanceReceivableQuery,
  FinanceSummary,
  UpdateFinanceExpenseRequest,
  UpdateFinanceReceivableRequest,
} from "./financeTypes";

export type FinanceApiSurface = {
  listFinanceExpenses: (
    query?: FinanceExpenseQuery,
  ) => Promise<FinanceExpense[]>;
  createFinanceExpense: (
    request: CreateFinanceExpenseRequest,
  ) => Promise<FinanceExpense>;
  updateFinanceExpense: (
    expenseId: string,
    request: UpdateFinanceExpenseRequest,
  ) => Promise<FinanceExpense>;
  listFinanceReceivables: (
    query?: FinanceReceivableQuery,
  ) => Promise<FinanceReceivable[]>;
  createFinanceReceivable: (
    request: CreateFinanceReceivableRequest,
  ) => Promise<FinanceReceivable>;
  updateFinanceReceivable: (
    receivableId: string,
    request: UpdateFinanceReceivableRequest,
  ) => Promise<FinanceReceivable>;
  getFinanceSummary: (taxYear?: number) => Promise<FinanceSummary>;
};

class FinanceApiClient implements FinanceApiSurface {
  private readonly baseUrl: string;
  private readonly options: ApiClientOptions;

  constructor(options: ApiClientOptions) {
    this.options = options;
    this.baseUrl = (options.baseUrl ?? config.apiBaseUrl).replace(/\/$/, "");
  }

  listFinanceExpenses(query: FinanceExpenseQuery = {}) {
    const params = new URLSearchParams();
    if (query.tax_year !== undefined) {
      params.set("tax_year", String(query.tax_year));
    }
    if (query.status) {
      params.set("status", query.status);
    }
    if (query.category) {
      params.set("category", query.category);
    }
    if (query.limit !== undefined) {
      params.set("limit", String(query.limit));
    }
    return this.request<FinanceExpense[]>(
      `/finance/expenses${queryString(params)}`,
    );
  }

  createFinanceExpense(request: CreateFinanceExpenseRequest) {
    return this.request<FinanceExpense>("/finance/expenses", {
      method: "POST",
      body: request,
    });
  }

  updateFinanceExpense(
    expenseId: string,
    request: UpdateFinanceExpenseRequest,
  ) {
    return this.request<FinanceExpense>(
      `/finance/expenses/${encodeURIComponent(expenseId)}`,
      { method: "PATCH", body: request },
    );
  }

  listFinanceReceivables(query: FinanceReceivableQuery = {}) {
    const params = new URLSearchParams();
    if (query.contact_id) {
      params.set("contact_id", query.contact_id);
    }
    if (query.status) {
      params.set("status", query.status);
    }
    if (query.limit !== undefined) {
      params.set("limit", String(query.limit));
    }
    return this.request<FinanceReceivable[]>(
      `/finance/receivables${queryString(params)}`,
    );
  }

  createFinanceReceivable(request: CreateFinanceReceivableRequest) {
    return this.request<FinanceReceivable>("/finance/receivables", {
      method: "POST",
      body: request,
    });
  }

  updateFinanceReceivable(
    receivableId: string,
    request: UpdateFinanceReceivableRequest,
  ) {
    return this.request<FinanceReceivable>(
      `/finance/receivables/${encodeURIComponent(receivableId)}`,
      { method: "PATCH", body: request },
    );
  }

  getFinanceSummary(taxYear?: number) {
    const params = new URLSearchParams();
    if (taxYear !== undefined) {
      params.set("tax_year", String(taxYear));
    }
    return this.request<FinanceSummary>(
      `/finance/summary${queryString(params)}`,
    );
  }

  private request<T>(path: string, requestOptions = {}) {
    return authenticatedRequest<T>({
      baseUrl: this.baseUrl,
      clientOptions: this.options,
      path,
      requestOptions,
    });
  }
}

export function attachFinanceApi<T extends object>(
  baseClient: T,
  options: ApiClientOptions,
): T & FinanceApiSurface {
  const finance = new FinanceApiClient(options);
  return Object.assign(baseClient, bindFinanceApi(finance));
}

function bindFinanceApi(finance: FinanceApiClient): FinanceApiSurface {
  return {
    listFinanceExpenses: (query) => finance.listFinanceExpenses(query),
    createFinanceExpense: (request) => finance.createFinanceExpense(request),
    updateFinanceExpense: (expenseId, request) =>
      finance.updateFinanceExpense(expenseId, request),
    listFinanceReceivables: (query) => finance.listFinanceReceivables(query),
    createFinanceReceivable: (request) =>
      finance.createFinanceReceivable(request),
    updateFinanceReceivable: (receivableId, request) =>
      finance.updateFinanceReceivable(receivableId, request),
    getFinanceSummary: (taxYear) => finance.getFinanceSummary(taxYear),
  };
}

function queryString(params: URLSearchParams) {
  const value = params.toString();
  return value ? `?${value}` : "";
}
